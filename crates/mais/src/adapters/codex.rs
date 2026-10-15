//! Codex session logs: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`. One `token_usage_record` per
//! model response (its `input_tokens` include the cached ones); tool calls and outputs joined by
//! `call_id`; plan readings from `token_count.rate_limits`.
use super::{Pending, Session, ms, target_of};
use crate::model::{Agent, Call, ItemKind, PlanReading, Runtime, Usage};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::Path;

const QUIET: &[&str] = &["turn_context", "world_state", "event_msg"];

#[derive(Default)]
struct State {
    id: String,
    s: Session,
    pending: Pending,
    tools: HashMap<String, (String, Option<String>, i64)>,
    /// The time of the latest line before the coming model response: its request went out then.
    input_ms: Option<i64>,
    now: i64,
    responses: HashSet<String>,
    reply: usize,
    folder: Option<String>,
    job_seen: bool,
}

pub fn read(file: &Path) -> std::io::Result<Session> {
    let mut st = State::default();
    for line in std::io::BufReader::new(std::fs::File::open(file)?).lines() {
        match serde_json::from_str::<Value>(&line?) {
            Ok(v) => st.line(&v),
            Err(_) => st.s.skipped += 1,
        }
    }
    if st.id.is_empty() {
        st.id = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    }
    Ok(st.finish())
}

impl State {
    /// Puts the session's id on everything it read.
    fn finish(mut self) -> Session {
        for c in &mut self.s.calls {
            c.agent.clone_from(&self.id);
        }
        for i in &mut self.s.items {
            i.agent.clone_from(&self.id);
        }
        for r in &mut self.s.readings {
            r.reporter = Some(self.id.clone());
        }
        self.s.agent = Some(Agent {
            id: self.id.clone(),
            name: self.id.clone(),
            group: None,
            role: None,
            runtime: Runtime::Codex,
            session: self.id.clone(),
            parent: None,
            folder: self.folder.take(),
            cli_cost_usd: None,
        });
        self.s
    }

    fn line(&mut self, v: &Value) {
        let p = &v["payload"];
        let at = v["timestamp"].as_str().and_then(ms).unwrap_or(0);
        self.now = at;
        match v["type"].as_str().unwrap_or("") {
            "session_meta" => self.meta(p),
            "token_usage_record" => self.usage(p, at),
            "response_item" => self.item(p),
            "compacted" => self.s.compactions.push(self.s.calls.len() as u32),
            "event_msg" if p["type"] == "token_count" => self.limits(&p["rate_limits"], at),
            t if QUIET.contains(&t) => {}
            _ => self.s.skipped += 1,
        }
    }

    fn meta(&mut self, p: &Value) {
        self.id = p["id"].as_str().unwrap_or_default().to_string();
        self.folder = p["cwd"].as_str().map(String::from);
        let base = p["base_instructions"]["text"].as_str().unwrap_or_default();
        self.pending.add_text("", ItemKind::SystemPrompt, base, "base instructions");
    }

    fn usage(&mut self, p: &Value, at: i64) {
        let response = p["response_id"].as_str().unwrap_or_default().to_string();
        if !response.is_empty() && !self.responses.insert(response) {
            return;
        }
        let u = &p["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let usage = Usage {
            input: n("input_tokens").saturating_sub(n("cached_input_tokens")),
            cache_read: n("cached_input_tokens"),
            cache_write: n("cache_write_input_tokens"),
            output: n("output_tokens"),
        };
        let seq = self.s.calls.len() as u32;
        self.pending.settle(seq, &mut self.s.items);
        let started = self.input_ms.take();
        self.s.calls.push(Call {
            agent: String::new(),
            seq,
            at_ms: at,
            model: None,
            usage,
            started_ms: started,
            ended_ms: Some(at),
        });
        let chars = std::mem::take(&mut self.reply);
        self.pending.add("", ItemKind::Reply, chars, "its own reply", None, None);
    }

    fn item(&mut self, p: &Value) {
        match p["type"].as_str().unwrap_or("") {
            "message" => self.message(p),
            "custom_tool_call" => {
                let input = p["input"].as_str();
                let target = if p["name"] == "apply_patch" { input.map(patch_files) } else { input.map(exec_target) };
                self.call(p, target);
            }
            "function_call" => {
                let args: Value =
                    p["arguments"].as_str().and_then(|a| serde_json::from_str(a).ok()).unwrap_or_default();
                self.call(p, target_of(&args));
            }
            "custom_tool_call_output" | "function_call_output" => self.output(p),
            _ => {}
        }
    }

    fn call(&mut self, p: &Value, target: Option<String>) {
        let name = p["name"].as_str().unwrap_or("?").to_string();
        self.reply += p["input"].as_str().map_or_else(|| p["arguments"].as_str().map_or(0, str::len), str::len);
        self.tools.insert(p["call_id"].as_str().unwrap_or_default().to_string(), (name, target, self.now));
    }

    fn output(&mut self, p: &Value) {
        let chars = text_of(&p["output"]).len();
        let (tool, target, asked) =
            self.tools.get(p["call_id"].as_str().unwrap_or_default()).cloned().unwrap_or(("?".into(), None, self.now));
        self.pending.add("", ItemKind::ToolResult, chars, "result", Some(tool), target);
        self.pending.took(Some(self.now - asked));
        self.input_ms = Some(self.now);
    }

    fn message(&mut self, p: &Value) {
        let text = text_of(&p["content"]);
        match p["role"].as_str().unwrap_or("") {
            "assistant" => self.reply += text.len(),
            "developer" => self.pending.add_text("", ItemKind::SystemPrompt, &text, "developer instructions"),
            "user" => {
                self.user(&text);
                self.input_ms = Some(self.now);
            }
            _ => {}
        }
    }

    fn user(&mut self, text: &str) {
        let (kind, label) = if text.starts_with("<environment_context>") {
            (ItemKind::SystemPrompt, "environment")
        } else if text.starts_with("# AGENTS.md") || text.starts_with("<user_instructions>") {
            (ItemKind::Memory, "AGENTS.md")
        } else if !std::mem::replace(&mut self.job_seen, true) {
            self.s.first_message = text.to_string();
            (ItemKind::Job, "first message")
        } else {
            (ItemKind::Message, "message")
        };
        self.pending.add("", kind, text.len(), label, None, None);
    }

    fn limits(&mut self, r: &Value, at: i64) {
        for slot in ["primary", "secondary"] {
            let w = &r[slot];
            let Some(percent) = w["used_percent"].as_f64() else { continue };
            let window = match w["window_minutes"].as_u64() {
                Some(300) => "five_hour".to_string(),
                Some(10080) => "seven_day".to_string(),
                Some(m) => format!("{m}_minutes"),
                None => continue,
            };
            self.s.readings.push(PlanReading {
                provider: "codex".into(),
                window,
                percent,
                resets_at: w["resets_at"].as_i64(),
                at_ms: at,
                reporter: None,
            });
        }
    }
}

/// Text of a string or a list of `{type, text}` parts.
fn text_of(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_else(|| {
        v.as_array().into_iter().flatten().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("\n")
    })
}

/// The first shell command in a Codex `exec` script, else its start.
fn exec_target(script: &str) -> String {
    let s = match script.find("cmd:\"") {
        Some(i) => script[i + 5..].split('"').next().unwrap_or_default(),
        None => script,
    };
    super::scrub(&s.chars().take(400).collect::<String>())
}

/// The files a patch touches (never its text, which can hold file contents and secrets).
fn patch_files(patch: &str) -> String {
    const HEADS: [&str; 3] = ["*** Update File: ", "*** Add File: ", "*** Delete File: "];
    let files = patch.lines().filter_map(|l| HEADS.iter().find_map(|h| l.strip_prefix(h)));
    files.map(str::trim).collect::<Vec<_>>().join(", ")
}
