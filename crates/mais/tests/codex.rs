use mais::adapters::codex;
use mais::model::*;
use std::path::PathBuf;

fn fixture() -> mais::adapters::Session {
    codex::read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/codex/rollout-1.jsonl")).unwrap()
}

#[test]
fn each_response_is_one_call_and_cached_input_is_counted_apart() {
    let s = fixture();
    assert_eq!(s.calls.len(), 3, "a repeated response is counted once");
    assert_eq!(s.calls[1].usage, Usage { input: 7000, cache_read: 19000, cache_write: 0, output: 80 });
    assert!(s.calls.iter().all(|c| c.agent == "cx-1"));
    assert_eq!(s.agent.as_ref().unwrap().folder.as_deref(), Some("/work/c172"));
}

#[test]
fn the_start_tools_and_job_are_found() {
    let s = fixture();
    let start: Vec<ItemKind> = s.items.iter().filter(|i| i.first_call == 0).map(|i| i.kind).collect();
    assert_eq!(start, [ItemKind::SystemPrompt, ItemKind::SystemPrompt, ItemKind::SystemPrompt, ItemKind::Job]);
    assert!(s.first_message.contains("Your role: Reviewer 8 [rev8]"));
    let r: Vec<_> = s.items.iter().filter(|i| i.kind == ItemKind::ToolResult).collect();
    assert_eq!(
        (r[0].first_call, r[0].chars, r[0].tool.as_deref(), r[0].target.as_deref()),
        (1, 5000, Some("exec"), Some("sed -n 1,400p src/build.py"))
    );
    assert_eq!(r[1].tool.as_deref(), Some("team_status"));
}

#[test]
fn plan_readings_compaction_and_unknown_lines() {
    let s = fixture();
    let r: Vec<(&str, f64)> = s.readings.iter().map(|r| (r.window.as_str(), r.percent)).collect();
    assert_eq!(r, [("five_hour", 22.0), ("seven_day", 70.0)]);
    assert_eq!(s.readings[0].reporter.as_deref(), Some("cx-1"));
    assert_eq!(s.compactions, [2]);
    assert_eq!(s.skipped, 1);
}

#[test]
fn a_response_starts_at_the_latest_input_and_a_tool_runs_from_call_to_output() {
    let s = fixture();
    assert_eq!(
        s.calls[0].ended_ms.zip(s.calls[0].started_ms).map(|(e, b)| e - b),
        Some(120_000),
        "the job at 22:10, the answer at 22:12"
    );
    let r = s.items.iter().find(|i| i.tool.as_deref() == Some("exec")).unwrap();
    assert_eq!(r.took_ms, Some(60_000));
}
