//! The run in figures, and one row per agent.
use super::tokens::Sized;
use crate::model::{Figure, Run, Usage};
use crate::report::{AgentRow, RunSummary};

fn median(mut v: Vec<u64>) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

pub fn run_summary(run: &Run, most_working: u32) -> RunSummary {
    let usage: Usage = run.calls.iter().map(|c| c.usage).sum();
    let mut models: Vec<String> = run.calls.iter().filter_map(|c| c.model.clone()).collect();
    models.sort();
    models.dedup();
    let starts = run.agents.iter().filter_map(|a| run.calls_of(&a.id).min_by_key(|c| c.seq).map(|c| c.usage.context())).collect();
    RunSummary {
        name: run.name.clone(),
        started_ms: run.calls.iter().map(|c| c.started_ms.unwrap_or(c.at_ms)).min(),
        ended_ms: run.calls.iter().map(|c| c.ended_ms.unwrap_or(c.at_ms)).max(),
        agents: run.agents.len() as u32,
        calls: run.calls.len() as u32,
        tokens: usage.total(),
        usage,
        most_working,
        models,
        starting_context: median(starts).map_or(Figure::unknown(), Figure::measured),
    }
}

pub fn agent_rows(run: &Run, items: &[Sized], times: &super::time::AgentTimes) -> Vec<AgentRow> {
    let mut rows: Vec<AgentRow> = run.agents.iter().map(|a| {
        let mut calls: Vec<_> = run.calls_of(&a.id).collect();
        calls.sort_by_key(|c| c.seq);
        let usage: Usage = calls.iter().map(|c| c.usage).sum();
        let first_seq = calls.first().map_or(0, |c| c.seq);
        let mine = items.iter().filter(|s| s.agent == a.id);
        let (start, growth): (Vec<&Sized>, Vec<&Sized>) = mine.partition(|s| s.first_call == first_seq && s.kind != crate::model::ItemKind::Message);
        let t = times.get(&a.id).cloned().unwrap_or_default();
        AgentRow {
            id: a.id.clone(), name: a.name.clone(), group: a.group.clone(), role: a.role.clone(), parent: a.parent.clone(), runtime: a.runtime,
            model: calls.iter().rev().find_map(|c| c.model.clone()),
            calls: calls.len() as u32, tokens: usage.total(), usage,
            starting_context: calls.first().map_or(0, |c| c.usage.context()),
            final_context: calls.last().map_or(0, |c| c.usage.context()),
            start_reread: start.iter().map(|s| s.reread()).sum(),
            growth_reread: growth.iter().map(|s| s.reread()).sum(),
            cli_cost_usd: a.cli_cost_usd,
            first_ms: calls.first().map(|c| c.started_ms.unwrap_or(c.at_ms)),
            last_ms: calls.last().map(|c| c.ended_ms.unwrap_or(c.at_ms)),
            model_ms: t.model, tool_ms: t.tools, waiting_ms: t.waiting,
        }
    }).collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.tokens));
    rows
}
