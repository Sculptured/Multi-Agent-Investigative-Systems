//! What agents had in common at their start (a prompt cache could hold it once), and the tools they
//! were given against the tools they used.
use super::tokens::Sized;
use crate::model::{Figure, ItemKind, Run};
use crate::report::{SharedPart, SharedStart, ToolUse};
use std::collections::{BTreeMap, BTreeSet};

/// Start items present, identical, in at least two agents.
pub fn shared_start(run: &Run, items: &[Sized]) -> SharedStart {
    let firsts: BTreeMap<&str, u32> = run.agents.iter().filter_map(|a| Some((a.id.as_str(), run.calls_of(&a.id).map(|c| c.seq).min()?))).collect();
    // (kind, fingerprint) → (label, agents that had it, its sizes)
    type Copies<'a> = (String, BTreeSet<&'a str>, Vec<u64>);
    let mut by_print: BTreeMap<(ItemKind, u64), Copies> = BTreeMap::new();
    for s in items.iter().filter(|s| firsts.get(s.agent.as_str()) == Some(&s.first_call)) {
        let Some(fp) = s.fingerprint else { continue };
        let e = by_print.entry((s.kind, fp)).or_insert_with(|| (s.label.clone(), BTreeSet::new(), vec![]));
        e.1.insert(&s.agent);
        e.2.push(s.tokens);
    }
    let mut parts: Vec<SharedPart> = by_print.into_iter().filter(|(_, (_, a, _))| a.len() >= 2).map(|((kind, _), (label, agents, sizes))| {
        let mean = sizes.iter().sum::<u64>() / sizes.len() as u64;
        SharedPart { kind, label, agents: agents.len() as u32, tokens: Figure::estimated(mean) }
    }).collect();
    parts.sort_by_key(|p| std::cmp::Reverse(p.tokens.value));
    let agents = parts.iter().map(|p| p.agents).max().unwrap_or(0);
    let per_agent: u64 = parts.iter().filter_map(|p| p.tokens.value).sum();
    let first_calls: Vec<&crate::model::Call> = firsts.iter().filter_map(|(a, s)| run.calls_of(a).find(|c| c.seq == *s)).collect();
    let writes: u64 = first_calls.iter().map(|c| c.usage.cache_write).sum();
    // The least the first calls could have written: the identical part once, and each agent's own rest.
    let shared_agents = u64::from(agents);
    let own: u64 = first_calls.iter().map(|c| c.usage.context().saturating_sub(if shared_agents > 1 { per_agent } else { 0 })).sum();
    let least = own + if shared_agents > 1 { per_agent } else { 0 };
    let avoidable = writes.saturating_sub(least);
    SharedStart { agents, parts, identical_per_agent: Figure::estimated(per_agent), first_call_cache_writes: writes, writes_avoidable: Figure::estimated(avoidable) }
}

/// Each tool: who was given it, what its definition cost in re-reads, and how often it was called.
pub fn tools(run: &Run, items: &[Sized]) -> Vec<ToolUse> {
    let calls_of_agent = |a: &str| run.calls_of(a).count() as u64;
    // Characters to tokens, per agent, from its sized tool definitions.
    let ratio: BTreeMap<&str, f64> = items.iter().filter(|s| s.kind == ItemKind::ToolDefinitions).filter_map(|s| {
        let chars: u64 = run.tools.iter().filter(|t| t.agent == s.agent).map(|t| t.chars).sum();
        (chars > 0).then(|| (s.agent.as_str(), s.tokens as f64 / chars as f64))
    }).collect();
    let mut by_tool: BTreeMap<&str, (u32, u64, u64)> = BTreeMap::new();
    for t in &run.tools {
        let r = ratio.get(t.agent.as_str()).copied().unwrap_or(0.25);
        let tokens = (t.chars as f64 * r).round() as u64;
        let e = by_tool.entry(&t.name).or_default();
        e.0 += 1;
        e.1 = e.1.max(tokens);
        e.2 += tokens * calls_of_agent(&t.agent);
    }
    let used = |name: &str| items.iter().filter(|s| s.kind == ItemKind::ToolResult && s.tool.as_deref() == Some(name)).count() as u32;
    let mut v: Vec<ToolUse> = by_tool.into_iter().map(|(name, (given, tokens, reread))| ToolUse {
        tool: name.to_string(), agents_given: given, definition_tokens: Figure::estimated(tokens), definition_reread: Figure::estimated(reread), calls: used(name),
    }).collect();
    v.sort_by_key(|t| std::cmp::Reverse(t.definition_reread.value));
    v
}
