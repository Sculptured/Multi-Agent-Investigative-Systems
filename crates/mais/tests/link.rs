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

fn session(id: &str, at_ms: i64) -> mais::adapters::Session {
    let agent = Agent {
        id: id.into(),
        name: id.into(),
        group: None,
        role: None,
        runtime: Runtime::ClaudeCode,
        session: id.into(),
        parent: None,
        folder: None,
        cli_cost_usd: None,
    };
    let usage = Usage { input: 1, cache_read: 0, cache_write: 100, output: 5 };
    let calls = (0..2)
        .map(|seq| Call {
            agent: id.into(),
            seq,
            at_ms: at_ms + i64::from(seq),
            model: None,
            usage,
            started_ms: None,
            ended_ms: None,
        })
        .collect();
    mais::adapters::Session {
        agent: Some(agent),
        calls,
        first_message: "Your role: Developer 8 [dev8]".into(),
        ..Default::default()
    }
}

#[test]
fn a_member_that_ran_two_sessions_gets_a_row_for_each() {
    let run = assemble(
        vec![session("s1", 1_000), session("s2", 5_000)],
        None,
        None,
        &Selection { since_ms: Some(0), ..Default::default() },
    );
    let ids: Vec<(&str, &str)> = run.agents.iter().map(|a| (a.id.as_str(), a.name.as_str())).collect();
    assert_eq!(ids, [("dev8", "Developer 8"), ("dev8~2", "Developer 8 (session 2)")]);
    assert_eq!(run.calls_of("dev8~2").count(), 2);
    assert_eq!(mais::analyze::analyze(&run).run.tokens, 4 * 106);
}

fn s1_item(first_call: u32, kind: ItemKind, chars: u64, label: &str) -> ContextItem {
    let tokens = Figure::unknown();
    ContextItem {
        agent: "s1".into(),
        first_call,
        kind,
        chars,
        tokens,
        label: label.into(),
        tool: None,
        target: None,
        fingerprint: None,
        took_ms: None,
    }
}

/// A session with a compaction before call 2: tool definitions and a job, a large result, then a small one.
fn compacted_session() -> mais::adapters::Session {
    let mut s = session("s1", 0);
    let usage = |context: u64| Usage { input: 0, cache_read: context, cache_write: 0, output: 10 };
    s.calls = [(0, 10_000), (1, 50_000), (2, 12_000), (3, 15_000)]
        .iter()
        .map(|&(seq, ctx)| Call {
            agent: "s1".into(),
            seq,
            at_ms: i64::from(seq) * 1_000,
            model: None,
            usage: usage(ctx),
            started_ms: None,
            ended_ms: None,
        })
        .collect();
    s.items = vec![
        s1_item(0, ItemKind::ToolDefinitions, 4_000, "tools"),
        s1_item(0, ItemKind::Job, 6_000, "job"),
        s1_item(1, ItemKind::ToolResult, 40_000, "result"),
        s1_item(3, ItemKind::ToolResult, 3_000, "result"),
    ];
    s.compactions = vec![2];
    s
}

#[test]
fn a_session_resumed_into_a_run_after_a_compaction_still_accounts_for_every_token() {
    let s = compacted_session();
    // The run starts at call 2, after the compaction (a session resumed into a new run).
    let run = assemble(vec![s], None, None, &Selection { since_ms: Some(1_500), ..Default::default() });
    let r = mais::analyze::analyze(&run);
    let parts: u64 = r.where_tokens_went.iter().map(|p| p.tokens).sum();
    assert_eq!(r.run.tokens, 12_010 + 15_010);
    assert!(parts.abs_diff(r.run.tokens) <= 10, "every token has a cause: {parts} of {}", r.run.tokens);
}
