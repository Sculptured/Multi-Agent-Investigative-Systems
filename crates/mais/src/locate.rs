//! Finds an agent CLI's session file from the session's id, for callers that know which sessions a
//! run had (an orchestrator) rather than which folder holds them.
use std::path::{Path, PathBuf};

/// Day folders this many days either side of the run are searched.
const DAYS_AROUND: i64 = 1;

fn plain(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `<projects>/<any folder>/<id>.jsonl` (Claude Code: `~/.claude/projects`).
pub fn claude_session(projects: &Path, id: &str) -> Option<PathBuf> {
    if !plain(id) {
        return None;
    }
    let name = format!("{id}.jsonl");
    std::fs::read_dir(projects).ok()?.filter_map(Result::ok).map(|e| e.path().join(&name)).find(|p| p.is_file())
}

/// `<sessions>/YYYY/MM/DD/rollout-…-<id>.jsonl` (Codex: `~/.codex/sessions`), in the day folders around
/// `at_ms` (the run's start).
pub fn codex_session(sessions: &Path, id: &str, at_ms: i64) -> Option<PathBuf> {
    if !plain(id) {
        return None;
    }
    let suffix = format!("-{id}.jsonl");
    (-DAYS_AROUND..=DAYS_AROUND).filter_map(|d| day_folder(sessions, at_ms, d)).find_map(|dir| {
        let files = std::fs::read_dir(dir).ok()?.filter_map(Result::ok).map(|e| e.path());
        files.into_iter().find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(&suffix)))
    })
}

fn day_folder(sessions: &Path, at_ms: i64, days: i64) -> Option<PathBuf> {
    let t = chrono::DateTime::from_timestamp_millis(at_ms + days * 86_400_000)?.with_timezone(&chrono::Local);
    Some(sessions.join(t.format("%Y/%m/%d").to_string()))
}
