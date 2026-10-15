//! What each item is, as a cause a person can act on, and the sections built on causes.
use super::tokens::Sized;
use crate::model::{ItemKind, Provenance, Run};
use crate::report::{CostlyItem, Part, SharedRead};
use std::collections::{BTreeMap, BTreeSet};

const TEAM_TOOLS: &[&str] = &["team_status", "send_agent_message", "read_decisions", "workflow_status", "report", "spawn_agent", "dispatch_work", "complete_work", "request_input", "record_decision"];

/// The command a shell target runs, after any leading `cd …&&`.
fn command(target: &str) -> &str {
    let t = target.trim_start();
    let t = if t.starts_with("cd ") { t.split_once("&&").or_else(|| t.split_once(';')).map_or(t, |(_, r)| r.trim_start()) } else { t };
    t.split_whitespace().next().unwrap_or("")
}

fn shell(target: &str) -> &'static str {
    match command(target) {
        "cat" | "sed" | "head" | "tail" | "grep" | "rg" | "wc" | "awk" | "less" | "nl" | "jq" => "Files printed (cat, sed, grep…)",
        "python" | "python3" | "node" | "uv" => "Scripts (python, node)",
        "ls" | "find" | "tree" | "du" | "fd" => "Listing folders (ls, find)",
        _ => "Other shell commands",
    }
}

/// The cause an item belongs to.
pub fn cause(s: &Sized) -> String {
    match s.kind {
        ItemKind::SystemPrompt => "The CLI's system prompt and notes".into(),
        ItemKind::ToolDefinitions => "Tool definitions".into(),
        ItemKind::Setup => "Your own CLI setup (skills list, plugins, hooks)".into(),
        ItemKind::Memory => "Memory files (CLAUDE.md, AGENTS.md)".into(),
        ItemKind::Job => "The agent's job and team description".into(),
        ItemKind::Reply => "Its own replies".into(),
        ItemKind::Message if s.label.starts_with("summary") => "Summaries after compaction".into(),
        ItemKind::Message if s.label.starts_with("growth") => "Growth the logs do not show".into(),
        ItemKind::Message => "Messages from people and teammates".into(),
        ItemKind::ToolResult => tool_cause(s),
    }
}

fn tool_cause(s: &Sized) -> String {
    let tool = s.tool.as_deref().unwrap_or("?");
    let target = s.target.as_deref().unwrap_or("");
    match tool {
        "Bash" | "exec" | "exec_command" | "shell" => shell(target).into(),
        "Read" if s.label == "image" => "Page images and renders".into(),
        "Read" => "Files read with the Read tool".into(),
        "read_pdf" => "PDF text".into(),
        "Grep" | "Glob" => "Searches (Grep, Glob)".into(),
        "Write" | "Edit" | "MultiEdit" | "apply_patch" => "Writing files".into(),
        "WebSearch" | "WebFetch" => "Web".into(),
        t if TEAM_TOOLS.contains(&t) => "Team tools".into(),
        t => format!("Other tools: {t}"),
    }
}

/// Every token by cause: the starting context's parts, what grew, and output.
pub fn where_tokens_went(run: &Run, items: &[Sized]) -> Vec<Part> {
    let mut parts: BTreeMap<(String, String), (u64, Provenance)> = BTreeMap::new();
    for s in items {
        let group = if s.first_call == first_seq(run, &s.agent) && s.kind != ItemKind::Message { "start" } else { "growth" };
        let e = parts.entry((group.into(), cause(s))).or_insert((0, s.provenance));
        e.0 += s.reread();
        if s.provenance == Provenance::Estimated {
            e.1 = Provenance::Estimated;
        }
    }
    let output: u64 = run.calls.iter().map(|c| c.usage.output).sum();
    let mut out: Vec<Part> = parts.into_iter().map(|((group, part), (tokens, provenance))| Part { group, part, tokens, provenance }).collect();
    out.push(Part { group: "output".into(), part: "Written by the agents (output and thinking)".into(), tokens: output, provenance: Provenance::Measured });
    out.sort_by_key(|p| std::cmp::Reverse(p.tokens));
    out
}

fn first_seq(run: &Run, agent: &str) -> u32 {
    run.calls_of(agent).map(|c| c.seq).min().unwrap_or(0)
}

/// The items whose re-reads cost the most.
pub fn costliest(items: &[Sized], n: usize) -> Vec<CostlyItem> {
    let mut v: Vec<&Sized> = items.iter().filter(|s| s.kind != ItemKind::Reply).collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.reread()));
    v.into_iter().take(n).map(|s| CostlyItem {
        agent: s.agent.clone(), kind: s.kind, cause: cause(s), tool: s.tool.clone(), target: s.target.clone(),
        tokens: s.figure(), reread: crate::model::Figure { value: Some(s.reread()), provenance: s.provenance },
    }).collect()
}

/// Files a command or tool read: the path-like words of its target.
fn files(target: &str) -> Vec<String> {
    target.split(|c: char| c.is_whitespace() || c == ';' || c == '|' || c == '&' || c == '\'' || c == '"')
        .filter(|w| w.contains('.') && !w.starts_with('-') && w.len() > 2 && !w.contains('='))
        .filter(|w| w.rsplit('.').next().is_some_and(|ext| (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())))
        .map(|w| w.rsplit('/').next().unwrap_or(w).to_string())
        .collect()
}

/// Files several agents read into their own context, and what those copies were re-read for.
pub fn shared_reads(items: &[Sized], n: usize) -> Vec<SharedRead> {
    let mut by_file: BTreeMap<String, (BTreeSet<&str>, u64)> = BTreeMap::new();
    for s in items.iter().filter(|s| s.kind == ItemKind::ToolResult) {
        let fs = files(s.target.as_deref().unwrap_or(""));
        let share = s.reread() / fs.len().max(1) as u64;
        for f in fs {
            let e = by_file.entry(f).or_default();
            e.0.insert(&s.agent);
            e.1 += share;
        }
    }
    let mut v: Vec<SharedRead> = by_file.into_iter().filter(|(_, (a, _))| a.len() >= 2)
        .map(|(target, (a, r))| SharedRead { target, agents: a.len() as u32, reread: crate::model::Figure::derived(r) }).collect();
    v.sort_by(|a, b| b.agents.cmp(&a.agents).then(b.reread.value.cmp(&a.reread.value)));
    v.truncate(n);
    v
}
