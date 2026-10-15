use mais::adapters::{sculptured, team};
use mais::link::{Selection, assemble, read_logs};
use mais::model::*;
use std::path::PathBuf;

fn fx(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(p)
}

#[test]
fn a_session_names_its_member_and_the_team_names_and_groups_it() {
    let doc = madl::load(&fx("team/c172.madl.yaml")).unwrap();
    let t = team::from_madl(&doc);
    let (sessions, skipped) = read_logs(&[fx("claude/proj")]);
    let run = assemble(sessions, Some(&t), None, &Selection::default());
    let dev = run.agent("dev8").expect("the session that says 'Your role: … [dev8]'");
    assert_eq!((dev.name.as_str(), dev.group.as_deref()), ("Developer 8", Some("Fuel Systems")));
    let sub = run.agent("dev8/ax1").expect("its subagent");
    assert_eq!(sub.parent.as_deref(), Some("dev8"));
    assert!(run.calls.iter().all(|c| c.agent == "dev8" || c.agent == "dev8/ax1"));
    assert_eq!(run.budget.as_ref().unwrap().plan, [("claude".to_string(), "five_hour".to_string(), 25.0, 30.0)]);
    assert_eq!(run.edges.len(), 1);
    assert_eq!(skipped.values().sum::<u64>(), 2);
}

#[test]
fn a_run_export_links_sessions_and_brings_plan_readings_and_handoffs() {
    let x = sculptured::read(&std::fs::read_to_string(fx("team/run.sculptured-session.jsonl")).unwrap()).unwrap();
    let (sessions, _) = read_logs(&[fx("codex")]);
    let sel = Selection { since_ms: Some(0), ..Default::default() };
    // The export's window is the run; the fixture's Codex calls are from another day, so widen it.
    let x2 = sculptured::RunExport { started_ms: 0, ended_ms: None, ..x.clone() };
    let run = assemble(sessions, Some(&x.team), Some(&x2), &sel);
    let rev = run.agent("rev8").expect("the export says session cx-1 is rev8");
    assert_eq!(rev.name, "Reviewer 8");
    assert_eq!(run.readings.len(), 1, "the export's readings, not the session's own as well");
    assert!(run.edges.iter().any(|e| e.from == "dev8" && e.to == "rev8" && e.at_ms.is_some()), "the observed handoff");
    assert_eq!(run.budget.as_ref().unwrap().plan[0].3, 10.0);
}

#[test]
fn the_newest_run_is_picked_when_nothing_says_which() {
    let (sessions, _) = read_logs(&[fx("claude/proj"), fx("codex")]);
    let run = assemble(sessions, None, None, &Selection::default());
    assert!(
        run.agents.iter().all(|a| a.runtime == Runtime::ClaudeCode),
        "the Codex fixture is a day older: another run"
    );
}
