//! Small runs built by hand for the analysis tests.
#![allow(dead_code)]
use mais::model::*;

pub fn agent(id: &str) -> Agent {
    Agent { id: id.into(), name: id.into(), group: None, role: None, runtime: Runtime::ClaudeCode, session: id.into(), parent: None, folder: None, cli_cost_usd: None }
}

/// A call whose whole context was written to the cache (as a first call's is).
pub fn call(agent: &str, seq: u32, at_ms: i64, context: u64, output: u64) -> Call {
    Call { agent: agent.into(), seq, at_ms, model: Some("m".into()), usage: Usage { input: 0, cache_read: 0, cache_write: context, output }, started_ms: None, ended_ms: None }
}

pub fn item(agent: &str, first_call: u32, kind: ItemKind, chars: u64) -> ContextItem {
    ContextItem { agent: agent.into(), first_call, kind, chars, tokens: Figure::unknown(), label: format!("{kind:?}"), tool: None, target: None, fingerprint: None, took_ms: None }
}

pub fn result(agent: &str, first_call: u32, tool: &str, target: &str, chars: u64) -> ContextItem {
    ContextItem { tool: Some(tool.into()), target: Some(target.into()), label: "result".into(), ..item(agent, first_call, ItemKind::ToolResult, chars) }
}

pub fn reading(provider: &str, percent: f64, resets_at: i64, at_ms: i64) -> PlanReading {
    PlanReading { provider: provider.into(), window: "five_hour".into(), percent, resets_at: Some(resets_at), at_ms, reporter: None }
}
