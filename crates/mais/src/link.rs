//! Finds the session logs under the paths given, picks one run's sessions, and assembles a `Run`:
//! each session becomes a team member where that can be told, else an agent of its own.
use crate::adapters::sculptured::RunExport;
use crate::adapters::team::Team;
use crate::adapters::{claude, codex, Session};
use crate::model::{Agent, Run};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A gap this long between sessions separates two runs.
pub const RUN_GAP_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    /// Only sessions whose id starts with one of these.
    pub sessions: Vec<String>,
}

/// Every session log under `paths` (files or folders), read.
pub fn read_logs(paths: &[PathBuf]) -> (Vec<Session>, BTreeMap<String, u64>) {
    let (mut out, mut skipped) = (Vec::new(), BTreeMap::new());
    for f in paths.iter().flat_map(|p| files(p)) {
        let read = if is_codex(&f) { codex::read(&f).map(|s| vec![s]) } else { claude::read(&f) };
        let Ok(sessions) = read else { continue };
        let n: u64 = sessions.iter().map(|s| s.skipped).sum();
        if n > 0 {
            skipped.insert(f.display().to_string(), n);
        }
        out.extend(sessions);
    }
    (out, skipped)
}

fn files(p: &Path) -> Vec<PathBuf> {
    if p.is_file() {
        return vec![p.to_path_buf()];
    }
    let Ok(entries) = std::fs::read_dir(p) else { return vec![] };
    let mut out = Vec::new();
    for e in entries.filter_map(Result::ok).map(|e| e.path()) {
        let skip = e.file_name().is_some_and(|n| n == "subagents" || n == "tool-results");
        if e.is_dir() && !skip {
            out.extend(files(&e));
        } else if e.extension().is_some_and(|x| x == "jsonl") {
            out.push(e);
        }
    }
    out.sort();
    out
}

fn is_codex(f: &Path) -> bool {
    use std::io::BufRead;
    let first = std::fs::File::open(f).ok().and_then(|h| std::io::BufReader::new(h).lines().next()).and_then(Result::ok).unwrap_or_default();
    first.contains("\"session_meta\"")
}

fn span(s: &Session) -> Option<(i64, i64)> {
    Some((s.calls.iter().map(|c| c.at_ms).min()?, s.calls.iter().map(|c| c.at_ms).max()?))
}

/// The time window to keep: the export's run, the selection's bounds, else the newest run (sessions
/// that follow one another with no gap of `RUN_GAP_MS`).
pub fn window(sessions: &[Session], sel: &Selection, export: Option<&RunExport>) -> (i64, i64) {
    if let Some(x) = export {
        return (x.started_ms, x.ended_ms.unwrap_or(i64::MAX));
    }
    if sel.since_ms.is_some() || sel.until_ms.is_some() {
        return (sel.since_ms.unwrap_or(i64::MIN), sel.until_ms.unwrap_or(i64::MAX));
    }
    let mut spans: Vec<(i64, i64)> = sessions.iter().filter_map(span).collect();
    spans.sort();
    let Some(&(mut start, mut end)) = spans.last() else { return (i64::MIN, i64::MAX) };
    for &(s, e) in spans.iter().rev() {
        if e + RUN_GAP_MS < start {
            break;
        }
        start = start.min(s);
        end = end.max(e);
    }
    (start, end)
}

/// The team member a session ran, from the export, else from "Your role: Name [member]" in its
/// first message (which also gives a name).
fn member_of(s: &Session, export: Option<&RunExport>) -> Option<(String, Option<String>)> {
    let a = s.agent.as_ref()?;
    if a.parent.is_some() {
        return None;
    }
    if let Some(m) = export.and_then(|x| x.sessions.get(a.session.as_str())) {
        return Some((m.clone(), None));
    }
    let line = s.first_message.lines().find(|l| l.contains("Your role:"))?;
    let (before, after) = line.rsplit_once('[')?;
    let id = after.split(']').next()?;
    let name = before.split("Your role:").nth(1).map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    (!id.is_empty()).then(|| (id.to_string(), name))
}

/// One run from `sessions`, named and grouped by `team`, linked by `export`.
pub fn assemble(sessions: Vec<Session>, team: Option<&Team>, export: Option<&RunExport>, sel: &Selection) -> Run {
    let (from, to) = window(&sessions, sel, export);
    let keep = |s: &Session| s.calls.iter().any(|c| c.at_ms >= from && c.at_ms <= to);
    let wanted = |s: &Session| sel.sessions.is_empty() || s.agent.as_ref().is_some_and(|a| sel.sessions.iter().any(|p| a.session.starts_with(p)));
    let sessions: Vec<Session> = sessions.into_iter().filter(|s| keep(s) && wanted(s)).collect();
    let found: BTreeMap<String, (String, Option<String>)> = sessions.iter().filter_map(|s| Some((s.agent.as_ref()?.id.clone(), member_of(s, export)?))).collect();
    let rename: BTreeMap<String, String> = found.iter().map(|(k, (m, _))| (k.clone(), m.clone())).collect();
    let names: BTreeMap<String, String> = found.into_values().filter_map(|(m, n)| Some((m, n?))).collect();
    let id_of = |old: &str| -> String {
        match old.split_once('/') {
            Some((main, sub)) => format!("{}/{sub}", rename.get(main).map_or(main, String::as_str)),
            None => rename.get(old).cloned().unwrap_or_else(|| old.to_string()),
        }
    };
    let mut run = Run { name: team.and_then(|t| t.name.clone()), ..Default::default() };
    for s in sessions {
        add(&mut run, s, &id_of, team, (from, to));
    }
    for a in &mut run.agents {
        if team.and_then(|t| t.member(&a.id)).is_none()
            && let Some(n) = names.get(&a.id)
        {
            a.name.clone_from(n);
        }
    }
    finish(&mut run, team, export);
    run
}

fn add(run: &mut Run, s: Session, id_of: &dyn Fn(&str) -> String, team: Option<&Team>, (from, to): (i64, i64)) {
    let Some(a) = s.agent else { return };
    let id = id_of(&a.id);
    let member = team.and_then(|t| t.member(&id));
    run.agents.push(Agent { name: member.map_or(a.name.clone(), |m| m.name.clone()), group: member.and_then(|m| m.group.clone()), parent: a.parent.as_deref().map(id_of), id: id.clone(), ..a });
    let kept: Vec<u32> = s.calls.iter().filter(|c| c.at_ms >= from && c.at_ms <= to).map(|c| c.seq).collect();
    let (lo, hi) = (kept.first().copied().unwrap_or(0), kept.last().copied().unwrap_or(0));
    run.calls.extend(s.calls.into_iter().filter(|c| c.seq >= lo && c.seq <= hi).map(|c| crate::model::Call { agent: id.clone(), ..c }));
    run.items.extend(s.items.into_iter().filter(|i| i.first_call <= hi).map(|i| crate::model::ContextItem { agent: id.clone(), ..i }));
    run.compactions.extend(s.compactions.into_iter().map(|c| (id.clone(), c)));
    run.tools.extend(s.tools.into_iter().map(|t| crate::model::ToolDef { agent: id.clone(), ..t }));
    run.readings.extend(s.readings.into_iter().filter(|r| r.at_ms >= from && r.at_ms <= to).map(|r| crate::model::PlanReading { reporter: Some(id.clone()), ..r }));
}

fn finish(run: &mut Run, team: Option<&Team>, export: Option<&RunExport>) {
    if let Some(t) = team {
        run.edges.extend(t.edges.iter().cloned());
        run.budget.clone_from(&t.budget);
    }
    if let Some(x) = export {
        // The export's readings cover every agent's CLI; the sessions' own would count them twice.
        run.readings = x.readings.clone();
        run.edges.extend(x.team.edges.iter().cloned());
        run.budget_events.clone_from(&x.budget_events);
        if run.budget.is_none() {
            run.budget.clone_from(&x.team.budget);
        }
        for a in &mut run.agents {
            if let Some(p) = x.parents.get(&a.id) {
                a.parent = Some(p.clone());
            }
        }
    }
    run.readings.sort_by_key(|r| r.at_ms);
    run.calls.sort_by(|a, b| (a.at_ms, &a.agent, a.seq).cmp(&(b.at_ms, &b.agent, b.seq)));
}
