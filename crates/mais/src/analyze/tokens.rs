//! Every context item's size in tokens and how many calls re-sent it.
//!
//! - The first call's context is split among the items it holds by their share of characters
//!   (`estimated`): the logs give the call's measured total, not each part's.
//! - A later call's rise in context is split among the items added just before it (`derived`).
//! - A compaction keeps the persistent start (system prompt, tool definitions, setup, memory) and
//!   replaces the rest with a summary: an item of its own, sized as the rest of that call's context.
use crate::model::{AgentId, ContextItem, Figure, ItemKind, Provenance, Run};

/// An item with its size and the calls that sent it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sized {
    pub agent: AgentId,
    pub kind: ItemKind,
    pub label: String,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub first_call: u32,
    pub tokens: u64,
    pub provenance: Provenance,
    /// Calls whose context held it, from its first.
    pub sent: u32,
    pub fingerprint: Option<u64>,
}

impl Sized {
    pub fn reread(&self) -> u64 {
        self.tokens * u64::from(self.sent)
    }

    pub fn figure(&self) -> Figure<u64> {
        Figure { value: Some(self.tokens), provenance: self.provenance }
    }

    /// Part of the start that stays in context through a compaction.
    pub fn persistent(&self) -> bool {
        matches!(self.kind, ItemKind::SystemPrompt | ItemKind::ToolDefinitions | ItemKind::Setup | ItemKind::Memory)
    }
}

/// Every item of every agent, sized, plus a summary item at each compaction and an item for growth
/// the logs show nothing for.
pub fn size(run: &Run) -> Vec<Sized> {
    run.agents.iter().flat_map(|a| agent(run, &a.id)).collect()
}

fn agent(run: &Run, id: &str) -> Vec<Sized> {
    let mut calls: Vec<_> = run.calls_of(id).collect();
    calls.sort_by_key(|c| c.seq);
    let Some(first) = calls.first() else { return vec![] };
    let ctx: Vec<u64> = calls.iter().map(|c| c.usage.context()).collect();
    let seqs: Vec<u32> = calls.iter().map(|c| c.seq).collect();
    let compactions: Vec<u32> = run.compactions.iter().filter(|(a, _)| a == id).map(|(_, c)| *c).collect();
    let items: Vec<&ContextItem> = run.items.iter().filter(|i| i.agent == id).collect();
    let mut out = Vec::new();
    for (k, seq) in seqs.iter().enumerate() {
        let here: Vec<&ContextItem> = items.iter().copied().filter(|i| i.first_call == *seq || (k == 0 && i.first_call < first.seq)).collect();
        if k == 0 {
            out.extend(split(here, ctx[0], Provenance::Estimated));
        } else if compactions.contains(seq) {
            let kept: u64 = out.iter().filter(|s: &&Sized| s.persistent()).map(|s| s.tokens).sum();
            out.push(synthetic(id, *seq, "summary after compaction", ctx[k].saturating_sub(kept)));
        } else {
            let rise = ctx[k].saturating_sub(ctx[k - 1]);
            let sized = split(here, rise, Provenance::Derived);
            if sized.is_empty() && rise > 0 {
                out.push(synthetic(id, *seq, "growth the logs do not show", rise));
            }
            out.extend(sized);
        }
    }
    for s in &mut out {
        s.sent = sent(s, &seqs, &compactions);
    }
    out
}

/// `total` tokens shared among `items` by characters.
fn split(items: Vec<&ContextItem>, total: u64, provenance: Provenance) -> Vec<Sized> {
    let chars: u64 = items.iter().map(|i| i.chars).sum();
    if chars == 0 {
        return vec![];
    }
    items.into_iter().map(|i| Sized {
        agent: i.agent.clone(), kind: i.kind, label: i.label.clone(), tool: i.tool.clone(), target: i.target.clone(), first_call: i.first_call,
        tokens: (total as f64 * i.chars as f64 / chars as f64).round() as u64, provenance, sent: 0, fingerprint: i.fingerprint,
    }).collect()
}

fn synthetic(agent: &str, seq: u32, label: &str, tokens: u64) -> Sized {
    Sized { agent: agent.into(), kind: ItemKind::Message, label: label.into(), tool: None, target: None, first_call: seq, tokens, provenance: Provenance::Derived, sent: 0, fingerprint: None }
}

/// Calls from the item's first until the next compaction (the persistent start: until the end).
fn sent(s: &Sized, seqs: &[u32], compactions: &[u32]) -> u32 {
    let end = if s.persistent() { u32::MAX } else { compactions.iter().copied().filter(|c| *c > s.first_call).min().unwrap_or(u32::MAX) };
    seqs.iter().filter(|q| **q >= s.first_call && **q < end).count() as u32
}
