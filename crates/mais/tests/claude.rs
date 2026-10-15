use mais::adapters::claude;
use mais::model::*;
use std::path::PathBuf;

fn fixture() -> Vec<mais::adapters::Session> {
    claude::read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/claude/proj/s1.jsonl")).unwrap()
}

#[test]
fn a_message_spread_over_lines_is_one_call_at_its_largest() {
    let s = &fixture()[0];
    assert_eq!(s.calls.len(), 3);
    assert_eq!(s.calls[0].usage, Usage { input: 2, cache_read: 0, cache_write: 41000, output: 12 });
    assert_eq!(s.calls[0].model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(s.agent.as_ref().unwrap().cli_cost_usd, Some(1.5));
    assert_eq!(s.agent.as_ref().unwrap().folder.as_deref(), Some("/work/c172"));
}

#[test]
fn the_starting_context_is_split_into_what_it_holds() {
    let s = &fixture()[0];
    let start: Vec<(ItemKind, u64)> = s.items.iter().filter(|i| i.first_call == 0).map(|i| (i.kind, i.chars)).collect();
    assert!(start.contains(&(ItemKind::Setup, 1000)), "the skills list: {start:?}");
    assert!(start.contains(&(ItemKind::Memory, 300)));
    assert!(
        start.iter().any(|(k, _)| *k == ItemKind::SystemPrompt)
            && start.iter().any(|(k, _)| *k == ItemKind::ToolDefinitions)
    );
    assert!(start.iter().any(|(k, c)| *k == ItemKind::Job && *c > 200));
    assert!(s.first_message.contains("Your role: Developer 8 [dev8]"));
    assert!(
        s.items.iter().all(|i| i.tokens.provenance == Provenance::Unknown),
        "sizes in tokens come from the analyses"
    );
}

#[test]
fn tool_results_name_their_tool_and_target_and_a_stored_output_counts_its_preview() {
    let s = &fixture()[0];
    let r: Vec<_> = s.items.iter().filter(|i| i.kind == ItemKind::ToolResult).collect();
    assert_eq!(
        (r[0].first_call, r[0].chars, r[0].tool.as_deref(), r[0].target.as_deref()),
        (1, 3000, Some("Bash"), Some("cat src/build.py"))
    );
    assert_eq!(
        (r[1].first_call, r[1].tool.as_deref(), r[1].target.as_deref()),
        (2, Some("read_pdf"), Some("manual.pdf"))
    );
    assert!(r[1].chars < 2300, "the model saw the 2 KB preview, not the 38.7 KB file: {}", r[1].chars);
    assert!(s.items.iter().any(|i| i.kind == ItemKind::Reply && i.first_call == 1), "its own reply is re-sent");
}

#[test]
fn compaction_and_unknown_lines_are_recorded() {
    let s = &fixture()[0];
    assert_eq!(s.compactions, [2]);
    assert_eq!(s.skipped, 2, "one unknown kind, one line that is not JSON");
}

#[test]
fn a_subagent_is_its_own_agent_under_its_parent() {
    let all = fixture();
    let sub = &all[1];
    let a = sub.agent.as_ref().unwrap();
    assert_eq!((a.id.as_str(), a.parent.as_deref(), a.name.as_str()), ("s1/ax1", Some("s1"), "List files"));
    assert_eq!(sub.calls[0].usage.cache_write, 8000);
}

#[test]
fn a_call_knows_when_its_request_went_out_and_a_tool_how_long_it_ran() {
    let s = &fixture()[0];
    let c = &s.calls[0];
    assert_eq!(
        c.ended_ms.zip(c.started_ms).map(|(e, b)| e - b),
        Some(30_000),
        "request at :30, answer at the next minute"
    );
    assert_eq!(s.calls[1].started_ms, None, "no request line: unknown, not zero");
    let r = s.items.iter().find(|i| i.kind == ItemKind::ToolResult).unwrap();
    assert_eq!(r.took_ms, Some(0));
}
