//! Readers for what agent CLIs and orchestrators record. Each turns its files into `Session`s (one
//! agent's calls and context) or team facts; `crate::link` assembles them into a `Run`.
pub mod claude;
pub mod codex;
pub mod sculptured;
pub mod team;

use crate::model::{Agent, Call, ContextItem, ItemKind, PlanReading, ToolDef};

/// One agent's session as a CLI recorded it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub agent: Option<Agent>,
    pub calls: Vec<Call>,
    pub items: Vec<ContextItem>,
    /// Calls that follow a compaction.
    pub compactions: Vec<u32>,
    pub readings: Vec<PlanReading>,
    pub tools: Vec<ToolDef>,
    /// The agent's first message, to find which team member it is.
    pub first_message: String,
    /// Lines of a kind this reader does not know.
    pub skipped: u64,
}

/// Items waiting for the next call, which is the first to hold them.
#[derive(Default)]
pub(crate) struct Pending(Vec<ContextItem>);

impl Pending {
    pub(crate) fn add(&mut self, agent: &str, kind: ItemKind, chars: usize, label: &str, tool: Option<String>, target: Option<String>) {
        if chars == 0 {
            return;
        }
        self.0.push(ContextItem { agent: agent.to_string(), first_call: 0, kind, chars: chars as u64, tokens: crate::model::Figure::unknown(), label: label.to_string(), tool, target, fingerprint: None, took_ms: None });
    }

    /// Sets how long the latest item's tool ran.
    pub(crate) fn took(&mut self, ms: Option<i64>) {
        if let Some(last) = self.0.last_mut() {
            last.took_ms = ms.filter(|m| *m >= 0);
        }
    }

    /// `add`, keeping a fingerprint of `content` so identical parts can be found across agents.
    pub(crate) fn add_text(&mut self, agent: &str, kind: ItemKind, content: &str, label: &str) {
        self.add(agent, kind, content.len(), label, None, None);
        if let Some(last) = self.0.last_mut().filter(|_| !content.is_empty()) {
            last.fingerprint = Some(fingerprint(content));
        }
    }

    /// Hands the waiting items to call `seq`.
    pub(crate) fn settle(&mut self, seq: u32, into: &mut Vec<ContextItem>) {
        for mut i in self.0.drain(..) {
            i.first_call = seq;
            into.push(i);
        }
    }
}

/// FNV-1a over the text: stable across builds, enough to tell identical parts apart.
pub fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A tool's short name: `mcp__server__tool` → `tool`.
pub fn short_tool(name: &str) -> String {
    name.rsplit("__").next().unwrap_or(name).to_string()
}

/// Milliseconds since the Unix epoch from an RFC 3339 time.
pub(crate) fn ms(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts).ok().map(|t| t.timestamp_millis())
}

/// The text a tool was run on: a shell command, a file, a pattern or a query.
pub(crate) fn target_of(input: &serde_json::Value) -> Option<String> {
    ["command", "cmd", "file_path", "path", "pattern", "query", "url"].iter().find_map(|k| input[*k].as_str()).map(|s| s.chars().take(400).collect())
}
