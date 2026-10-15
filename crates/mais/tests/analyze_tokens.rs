mod common;
use common::*;
use mais::analyze::{causes, tokens};
use mais::model::*;

fn one_agent(contexts: &[u64]) -> Run {
    Run {
        agents: vec![agent("a")],
        calls: contexts.iter().enumerate().map(|(k, c)| call("a", k as u32, k as i64 * 1000, *c, 10)).collect(),
        ..Default::default()
    }
}

#[test]
fn the_first_calls_context_is_split_by_characters_as_an_estimate_and_re_read_on_every_call() {
    let mut run = one_agent(&[1000, 1000, 1000]);
    run.items = vec![item("a", 0, ItemKind::ToolDefinitions, 300), item("a", 0, ItemKind::Setup, 700)];
    let s = tokens::size(&run);
    assert_eq!(s.iter().map(|x| x.tokens).collect::<Vec<_>>(), [300, 700]);
    assert!(s.iter().all(|x| x.provenance == Provenance::Estimated && x.sent == 3));
    assert_eq!(s[1].reread(), 2100);
}

#[test]
fn a_later_rise_is_split_among_what_was_added_and_unexplained_growth_is_its_own_item() {
    let mut run = one_agent(&[1000, 1500, 1800]);
    run.items =
        vec![item("a", 0, ItemKind::Job, 10), result("a", 1, "Read", "a.md", 100), result("a", 1, "Read", "b.md", 300)];
    let s = tokens::size(&run);
    let read: Vec<(u64, Provenance, u32)> =
        s.iter().filter(|x| x.kind == ItemKind::ToolResult).map(|x| (x.tokens, x.provenance, x.sent)).collect();
    assert_eq!(read, [(125, Provenance::Derived, 2), (375, Provenance::Derived, 2)]);
    let unexplained = s.iter().find(|x| x.label.starts_with("growth")).expect("call 2 rose by 300 with nothing added");
    assert_eq!((unexplained.tokens, unexplained.first_call, unexplained.sent), (300, 2, 1));
}

#[test]
fn after_a_compaction_earlier_items_stop_costing_and_the_summary_starts_over() {
    let mut run = one_agent(&[1000, 3000, 1200, 1500]);
    run.items = vec![
        item("a", 0, ItemKind::ToolDefinitions, 500),
        item("a", 0, ItemKind::Job, 500),
        result("a", 1, "Bash", "cat big.md", 10),
    ];
    run.compactions = vec![("a".into(), 2)];
    let s = tokens::size(&run);
    let by = |label: &str| s.iter().find(|x| x.label.starts_with(label)).unwrap();
    assert_eq!(by("ToolDefinitions").sent, 4, "the tool definitions stay through a compaction");
    assert_eq!(by("Job").sent, 2, "the job was in calls 0 and 1, then summarized");
    assert_eq!((by("result").tokens, by("result").sent), (2000, 1));
    let summary = by("summary");
    assert_eq!((summary.tokens, summary.sent), (700, 2), "call 2's context less the 500 kept");
}

#[test]
fn causes_name_shell_reads_after_a_cd_and_files_read_by_several_agents() {
    let mut run = one_agent(&[100, 200]);
    run.agents.push(agent("b"));
    run.calls.extend([call("b", 0, 0, 100, 1), call("b", 1, 1000, 300, 1)]);
    run.items = vec![
        result("a", 1, "Bash", "cd /w && cat plans/PLAN.md | head", 50),
        result("b", 1, "Read", "/w/plans/PLAN.md", 50),
    ];
    let s = tokens::size(&run);
    let printed = s.iter().find(|x| x.tool.as_deref() == Some("Bash")).unwrap();
    assert_eq!(causes::cause(printed), "Files printed (cat, sed, grep…)");
    let shared = causes::shared_reads(&s, 5);
    assert_eq!((shared[0].target.as_str(), shared[0].agents), ("PLAN.md", 2));
}

#[test]
fn a_start_the_log_does_not_break_down_is_one_measured_part_not_a_guess() {
    let mut run = one_agent(&[20_000, 20_000]);
    // Claude Code before 2.1.300 logs no request shape: no system prompt or tool definitions, only attachments.
    run.items = vec![item("a", 0, ItemKind::Setup, 1_000), item("a", 0, ItemKind::Job, 17)];
    let s = tokens::size(&run);
    assert_eq!(s.len(), 1, "{s:?}");
    assert_eq!((s[0].tokens, s[0].provenance, s[0].sent), (20_000, Provenance::Measured, 2));
    assert_eq!(causes::cause(&s[0]), "Starting context the CLI's log does not break down");
    let r = mais::analyze::analyze(&run);
    assert!(!r.where_tokens_went.iter().any(|p| p.part.starts_with("Your own CLI setup")));
    assert!(
        r.findings.iter().any(|f| f.text.contains("starting context whose parts the CLI's log does not record")),
        "{:?}",
        r.findings
    );
}
