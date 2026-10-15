//! Claude Code session logs: `~/.claude/projects/<folder>/<session>.jsonl`, with subagents in
//! `<session>/subagents/agent-<id>.jsonl` (and `.meta.json`). Each API message's usage repeats on
//! every line of the message, its output growing: it is counted once, at its largest.
mod lines;

use super::Session;
use crate::model::{Agent, Runtime};
use std::path::Path;

/// The session in `file`, and its subagents as sessions of their own (each with its parent).
pub fn read(file: &Path) -> std::io::Result<Vec<Session>> {
    let id = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let mut main = lines::read(file, &id)?;
    let folder = main.agent.as_ref().and_then(|a| a.folder.clone());
    main.agent = Some(Agent {
        id: id.clone(),
        name: id.clone(),
        group: None,
        role: None,
        runtime: Runtime::ClaudeCode,
        session: id.clone(),
        parent: None,
        folder: folder.clone(),
        cli_cost_usd: main.agent.as_ref().and_then(|a| a.cli_cost_usd),
    });
    let mut out = vec![main];
    let dir = file.with_extension("").join("subagents");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(out) };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    for f in files {
        out.push(subagent(&f, &id, folder.clone())?);
    }
    Ok(out)
}

fn subagent(f: &Path, session: &str, folder: Option<String>) -> std::io::Result<Session> {
    let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let short = stem.trim_start_matches("agent-");
    let meta: serde_json::Value = std::fs::read_to_string(f.with_extension("meta.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let id = format!("{session}/{short}");
    let parent =
        meta["parentAgentId"].as_str().map(|p| format!("{session}/{p}")).unwrap_or_else(|| session.to_string());
    let name = meta["description"].as_str().or(meta["agentType"].as_str()).unwrap_or(short).to_string();
    let mut s = lines::read(f, &id)?;
    s.agent = Some(Agent {
        id: id.clone(),
        name,
        group: None,
        role: meta["agentType"].as_str().map(String::from),
        runtime: Runtime::ClaudeCode,
        session: session.to_string(),
        parent: Some(parent),
        folder,
        cli_cost_usd: None,
    });
    Ok(s)
}
