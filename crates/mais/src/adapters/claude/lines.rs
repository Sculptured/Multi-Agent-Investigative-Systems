//! One Claude Code log file, line by line.
use crate::adapters::{Pending, Session, ms, target_of};
use crate::model::{Agent, Call, ItemKind, Runtime, Usage};
use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

/// Line kinds that carry nothing MAIS needs.
const QUIET: &[&str] = &[
    "queue-operation",
    "last-prompt",
    "atis-latch",
    "api-request-blob",
    "summary",
    "file-history-snapshot",
    "custom-title",
    "agent-name",
    "permission-mode",
    "pr-link",
];

#[derive(Default)]
struct State {
    id: String,
    s: Session,
    pending: Pending,
    /// Message id → its call's index.
    calls: HashMap<String, usize>,
    /// Tool use id → (tool, target, when it was asked for).
    tools: HashMap<String, (String, Option<String>, i64)>,
    /// When the latest request went out (`api-request`), for the call it starts.
    request_ms: Option<i64>,
    /// The time of the line being read.
    now: i64,
    /// The latest message's reply, added to the next call's context.
    reply: (String, usize),
    shape_seen: bool,
    folder: Option<String>,
    cost: Option<f64>,
}

pub fn read(file: &Path, id: &str) -> std::io::Result<Session> {
    let mut st = State { id: id.to_string(), ..Default::default() };
    for line in std::io::BufReader::new(std::fs::File::open(file)?).lines() {
        let line = line?;
        match serde_json::from_str::<Value>(&line) {
            Ok(v) => st.line(&v),
            Err(_) => st.s.skipped += 1,
        }
    }
    st.s.agent = Some(Agent {
        id: st.id.clone(),
        name: st.id.clone(),
        group: None,
        role: None,
        runtime: Runtime::ClaudeCode,
        session: st.id.clone(),
        parent: None,
        folder: st.folder.clone(),
        cli_cost_usd: st.cost,
    });
    Ok(st.s)
}

impl State {
    fn line(&mut self, v: &Value) {
        if self.folder.is_none() {
            self.folder = v["cwd"].as_str().map(String::from);
        }
        if let Some(t) = v["timestamp"].as_str().and_then(ms) {
            self.now = t;
        }
        match v["type"].as_str().unwrap_or("") {
            "api-request" => self.request_ms = Some(self.now),
            "assistant" => self.assistant(v),
            "user" => self.user(v),
            "attachment" => self.attachment(v),
            "api-request-shape" => self.shape(v),
            "system" if v["subtype"] == "compact_boundary" => self.s.compactions.push(self.s.calls.len() as u32),
            "system" => {}
            "cost-state" => self.cost = v["totalCostUSD"].as_f64(),
            t if QUIET.contains(&t) => {}
            _ => self.s.skipped += 1,
        }
    }

    fn assistant(&mut self, v: &Value) {
        let m = &v["message"];
        let Some(mid) = m["id"].as_str() else { return };
        let u = &m["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let usage = Usage {
            input: n("input_tokens"),
            cache_read: n("cache_read_input_tokens"),
            cache_write: n("cache_creation_input_tokens"),
            output: n("output_tokens"),
        };
        let at = v["timestamp"].as_str().and_then(ms).unwrap_or(0);
        let idx = match self.calls.get(mid) {
            Some(&i) => i,
            None => self.new_call(mid, at, m["model"].as_str()),
        };
        self.s.calls[idx].usage = self.s.calls[idx].usage.max(usage);
        self.s.calls[idx].ended_ms = Some(at);
        for b in m["content"].as_array().into_iter().flatten() {
            self.reply.1 += reply_chars(b);
            if b["type"] == "tool_use" {
                let name = crate::adapters::short_tool(b["name"].as_str().unwrap_or("?"));
                self.tools.insert(b["id"].as_str().unwrap_or_default().to_string(), (name, target_of(&b["input"]), at));
            }
        }
    }

    fn new_call(&mut self, mid: &str, at: i64, model: Option<&str>) -> usize {
        let seq = self.s.calls.len() as u32;
        let (_, chars) = std::mem::take(&mut self.reply);
        self.pending.add(&self.id.clone(), ItemKind::Reply, chars, "its own reply", None, None);
        self.pending.settle(seq, &mut self.s.items);
        let started = self.request_ms.take();
        self.s.calls.push(Call {
            agent: self.id.clone(),
            seq,
            at_ms: at,
            model: model.map(String::from),
            usage: Usage::default(),
            started_ms: started,
            ended_ms: Some(at),
        });
        self.calls.insert(mid.to_string(), seq as usize);
        seq as usize
    }

    fn user(&mut self, v: &Value) {
        let c = &v["message"]["content"];
        if let Some(text) = c.as_str() {
            return self.text(text);
        }
        for b in c.as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("tool_result") => self.tool_result(b),
                Some("text") => self.text(b["text"].as_str().unwrap_or_default()),
                _ => {}
            }
        }
    }

    fn text(&mut self, text: &str) {
        let first = self.s.calls.is_empty() && self.s.first_message.is_empty();
        if first {
            self.s.first_message = text.to_string();
        }
        let (kind, label) = if first { (ItemKind::Job, "first message") } else { (ItemKind::Message, "message") };
        self.pending.add(&self.id.clone(), kind, text.len(), label, None, None);
    }

    fn tool_result(&mut self, b: &Value) {
        let content = &b["content"];
        let chars = content.as_str().map_or_else(|| content_chars(content), str::len);
        let (tool, target, asked) = self.tools.get(b["tool_use_id"].as_str().unwrap_or_default()).cloned().unwrap_or((
            "?".into(),
            None,
            self.now,
        ));
        let label = if content.to_string().contains("\"type\":\"image\"") { "image" } else { "result" };
        self.pending.add(&self.id.clone(), ItemKind::ToolResult, chars, label, Some(tool), target);
        self.pending.took(Some(self.now - asked));
    }

    fn attachment(&mut self, v: &Value) {
        let a = &v["attachment"];
        let kind = match a["type"].as_str().unwrap_or("") {
            "skill_listing"
            | "mcp_instructions_delta"
            | "deferred_tools_delta"
            | "hook_additional_context"
            | "hook_success" => ItemKind::Setup,
            "instructions" => ItemKind::Memory,
            "queued_command"
            | "deferred_tools_record"
            | "prompt_snapshot"
            | "credential_org"
            | "remote_session_change" => return,
            _ => ItemKind::SystemPrompt,
        };
        let rendered: usize =
            v["rendered"].as_array().into_iter().flatten().map(|r| r["content"].as_str().map_or(0, str::len)).sum();
        // A hook's output is rendered once, as `hook_additional_context`.
        if a["type"] == "hook_success" {
            return;
        }
        let text: String = if rendered > 0 {
            v["rendered"].as_array().into_iter().flatten().filter_map(|r| r["content"].as_str()).collect()
        } else {
            a.to_string()
        };
        self.pending.add_text(&self.id.clone(), kind, &text, a["type"].as_str().unwrap_or("attachment"));
    }

    /// The first request's system prompt and tool definitions.
    fn shape(&mut self, v: &Value) {
        if std::mem::replace(&mut self.shape_seen, true) {
            return;
        }
        let sh = &v["shape"];
        let id = self.id.clone();
        self.pending.add_text(&id, ItemKind::SystemPrompt, &sh["system"].to_string(), "system prompt");
        self.pending.add_text(&id, ItemKind::ToolDefinitions, &sh["tools"].to_string(), "tool definitions");
        for t in sh["tools"].as_array().into_iter().flatten() {
            let name = crate::adapters::short_tool(t["name"].as_str().unwrap_or("?"));
            self.s.tools.push(crate::model::ToolDef { agent: id.clone(), name, chars: t.to_string().len() as u64 });
        }
    }
}

fn reply_chars(b: &Value) -> usize {
    match b["type"].as_str() {
        Some("text") => b["text"].as_str().map_or(0, str::len),
        Some("tool_use") => b["input"].to_string().len(),
        _ => 0,
    }
}

fn content_chars(c: &Value) -> usize {
    c.as_array().into_iter().flatten().map(|x| x["text"].as_str().map_or_else(|| x.to_string().len(), str::len)).sum()
}
