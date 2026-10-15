use super::{Figure, Usage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An agent's id in the run: its team member id when known, else its session id.
pub type AgentId = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Runtime {
    ClaudeCode,
    Codex,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub group: Option<String>,
    pub role: Option<String>,
    pub runtime: Runtime,
    /// The CLI's session id.
    pub session: String,
    /// The agent that started it (a subagent or a spawned child).
    pub parent: Option<AgentId>,
    /// Where it worked.
    pub folder: Option<String>,
    /// The CLI's own cost figure for the session, in US dollars at API prices.
    pub cli_cost_usd: Option<f64>,
}

/// One request to the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Call {
    pub agent: AgentId,
    /// 0-based, in order, within the agent.
    pub seq: u32,
    pub at_ms: i64,
    pub model: Option<String>,
    pub usage: Usage,
    /// When the request went out and when its answer was complete, where the log says.
    pub started_ms: Option<i64>,
    pub ended_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    SystemPrompt,
    ToolDefinitions,
    /// The person's own CLI setup loaded into the agent: skills list, plugin and MCP instructions,
    /// deferred tool names, hook output.
    Setup,
    /// CLAUDE.md, AGENTS.md, memory files.
    Memory,
    /// The agent's first message: its job and the team's description.
    Job,
    ToolResult,
    /// A later message from a person or a teammate.
    Message,
    /// The agent's own output, re-sent as history.
    Reply,
}

/// Something in an agent's context, from the call before which it was added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContextItem {
    pub agent: AgentId,
    /// The first call whose context holds it.
    pub first_call: u32,
    pub kind: ItemKind,
    pub chars: u64,
    pub tokens: Figure<u64>,
    /// What it is, in a few words (a tool name, an attachment's kind).
    pub label: String,
    pub tool: Option<String>,
    /// The command, file or query a tool result came from.
    pub target: Option<String>,
    /// A hash of its content, for the parts of a start that agents may share; None when not kept.
    pub fingerprint: Option<u64>,
    /// A tool result: how long the tool ran (from its call to its result), where the log says.
    pub took_ms: Option<i64>,
}

/// A tool an agent was given (its definition is sent on every call).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolDef {
    pub agent: AgentId,
    pub name: String,
    pub chars: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanReading {
    /// `claude` or `codex`.
    pub provider: String,
    /// `five_hour`, `seven_day`, or `<n>_minutes`.
    pub window: String,
    pub percent: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
    pub at_ms: i64,
    pub reporter: Option<AgentId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetEventKind {
    WrapUp,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BudgetEvent {
    pub kind: BudgetEventKind,
    pub at_ms: i64,
    pub reason: String,
}

/// A run budget's values, when the run had one (`soft` defaults as the orchestrator set them).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BudgetLimits {
    /// Per provider and window: (soft, hard) percent the run may add.
    pub plan: Vec<(String, String, f64, f64)>,
    pub tokens: Option<(u64, u64)>,
    pub minutes: Option<(u32, u32)>,
    pub max_working: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    HandsWorkTo,
    Spawned,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub from: AgentId,
    pub to: AgentId,
    pub kind: EdgeKind,
    /// When it happened, for a handoff or a spawn that was observed.
    pub at_ms: Option<i64>,
}

/// A run: everything the adapters found, normalised.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Run {
    pub name: Option<String>,
    pub agents: Vec<Agent>,
    pub calls: Vec<Call>,
    pub items: Vec<ContextItem>,
    /// (agent, the first call after a compaction): context items added before it stop being re-sent.
    pub compactions: Vec<(AgentId, u32)>,
    pub readings: Vec<PlanReading>,
    pub budget: Option<BudgetLimits>,
    pub budget_events: Vec<BudgetEvent>,
    pub edges: Vec<Edge>,
    /// The tools each agent was given.
    pub tools: Vec<ToolDef>,
    /// Lines no adapter understood, by file.
    pub skipped: BTreeMap<String, u64>,
}

impl Run {
    pub fn agent(&self, id: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn calls_of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Call> + 'a {
        self.calls.iter().filter(move |c| c.agent == id)
    }
}
