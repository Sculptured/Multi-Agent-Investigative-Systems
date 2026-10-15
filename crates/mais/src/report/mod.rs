//! The report (`mais.report/v1`): what a run used and why, every number with its provenance.
use crate::model::{AgentId, BudgetEvent, Figure, ItemKind, Runtime, Usage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT: &str = "mais.report/v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Report {
    /// Always "mais.report/v1".
    pub format: String,
    pub run: RunSummary,
    pub agents: Vec<AgentRow>,
    /// Every token of the run by cause: the starting context re-read, what grew, and output.
    pub where_tokens_went: Vec<Part>,
    /// What added the most to later calls' context, most first.
    pub costliest: Vec<CostlyItem>,
    /// Commands and files several agents read for themselves.
    pub shared_reads: Vec<SharedRead>,
    pub shared_start: SharedStart,
    pub tools: Vec<ToolUse>,
    pub timeline: Timeline,
    pub latency: Latency,
    pub plan: Vec<PlanSeries>,
    pub cost: Cost,
    pub findings: Vec<Finding>,
    /// Lines the readers did not know, by file.
    pub skipped: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    pub name: Option<String>,
    pub started_ms: Option<i64>,
    pub ended_ms: Option<i64>,
    pub agents: u32,
    pub calls: u32,
    pub usage: Usage,
    pub tokens: u64,
    /// The most agents making calls in one interval of the timeline.
    pub most_working: u32,
    pub models: Vec<String>,
    /// The median agent's first-call context.
    pub starting_context: Figure<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AgentRow {
    pub id: AgentId,
    pub name: String,
    pub group: Option<String>,
    pub role: Option<String>,
    pub parent: Option<AgentId>,
    pub runtime: Runtime,
    pub model: Option<String>,
    pub calls: u32,
    pub usage: Usage,
    pub tokens: u64,
    pub starting_context: u64,
    pub final_context: u64,
    /// Its starting context, re-read on every call.
    pub start_reread: u64,
    /// What it added while working, re-read on every later call (and summaries after compaction).
    pub growth_reread: u64,
    pub cli_cost_usd: Option<f64>,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    /// Time in model calls, in tools, and neither (waiting for work, answers or teammates).
    pub model_ms: Figure<i64>,
    pub tool_ms: Figure<i64>,
    pub waiting_ms: Figure<i64>,
}

/// One cause, in tokens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Part {
    /// `start`, `growth` or `output`.
    pub group: String,
    pub part: String,
    pub tokens: u64,
    pub provenance: crate::model::Provenance,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CostlyItem {
    pub agent: AgentId,
    pub kind: ItemKind,
    pub cause: String,
    pub tool: Option<String>,
    pub target: Option<String>,
    /// Its own size, and that size times the calls that re-sent it.
    pub tokens: Figure<u64>,
    pub reread: Figure<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SharedRead {
    pub target: String,
    pub agents: u32,
    pub reread: Figure<u64>,
}

/// The parts of the starting context that were identical across agents, which a provider's prompt
/// cache could hold once for all of them if they came first and in the same order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SharedStart {
    pub agents: u32,
    pub parts: Vec<SharedPart>,
    /// Tokens of each agent's start that were identical in every agent that had them.
    pub identical_per_agent: Figure<u64>,
    /// What the agents wrote to the cache on their first calls, together.
    pub first_call_cache_writes: u64,
    /// Cache writes that would have been reads had the identical part been written once.
    pub writes_avoidable: Figure<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SharedPart {
    pub kind: ItemKind,
    pub label: String,
    pub agents: u32,
    pub tokens: Figure<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolUse {
    pub tool: String,
    /// Agents it was given to, and the calls of those agents that re-sent its definition.
    pub agents_given: u32,
    pub definition_tokens: Figure<u64>,
    pub definition_reread: Figure<u64>,
    /// Times it was called.
    pub calls: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Timeline {
    pub interval_ms: i64,
    pub points: Vec<TimePoint>,
    pub fanouts: Vec<Fanout>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TimePoint {
    pub at_ms: i64,
    pub tokens: u64,
    pub agents_working: u32,
}

/// Many agents starting close together.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fanout {
    pub at_ms: i64,
    pub agents: u32,
    /// The agent that handed them work, where the run export says.
    pub from: Option<AgentId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Durations {
    pub count: u32,
    pub total_ms: i64,
    pub median_ms: Option<i64>,
    pub p90_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Latency {
    /// Model calls whose start and end the logs give.
    pub model: Durations,
    pub by_model: BTreeMap<String, Durations>,
    pub tools: BTreeMap<String, Durations>,
    /// Shares of the agents' active time: in model calls, in tools, waiting.
    pub model_share: Figure<f64>,
    pub tool_share: Figure<f64>,
    pub waiting_share: Figure<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlanSeries {
    pub provider: String,
    pub window: String,
    /// (time, reading, highest reading so far in this window).
    pub points: Vec<(i64, f64, f64)>,
    pub first: f64,
    pub highest: f64,
    /// Percentage points the run added: highest minus first (across a reset, the sum of both windows').
    pub added: Figure<f64>,
    /// Points per million tokens of this provider's agents.
    pub points_per_million: Figure<f64>,
    /// The run budget's (soft, hard) for this window, as points the run may add.
    pub budget: Option<(f64, f64)>,
    pub events: Vec<BudgetEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cost {
    /// The CLIs' own figures, summed (Claude Code reports one; Codex does not).
    pub cli_usd: Figure<f64>,
    /// Share of the cost by kind of token, at the usual price ratios (an estimate).
    pub by_kind: Vec<CostKind>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CostKind {
    pub kind: String,
    pub tokens: u64,
    /// Price relative to fresh input.
    pub weight: f64,
    pub share: f64,
}

/// One plain sentence a person can act on, with the section it rests on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub text: String,
    pub section: String,
}

/// The report's JSON Schema.
pub fn schema() -> schemars::Schema {
    schemars::schema_for!(Report)
}

/// Leaves out commands, file paths and message text, keeping kinds and sizes.
pub fn redact(r: &mut Report) {
    for c in &mut r.costliest {
        c.target = None;
    }
    for (i, s) in r.shared_reads.iter_mut().enumerate() {
        s.target = format!("file {}", i + 1);
    }
    r.findings.retain(|f| f.section != "shared_reads");
    r.skipped = r.skipped.values().enumerate().map(|(i, n)| (format!("file {}", i + 1), *n)).collect();
}
