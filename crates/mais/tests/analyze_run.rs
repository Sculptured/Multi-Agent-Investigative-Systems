mod common;
use common::*;
use mais::analyze::{analyze, plan, sharing, time, tokens};
use mais::model::*;
use proptest::prelude::*;

#[test]
fn plan_readings_out_of_order_use_the_highest_so_far_and_a_reset_starts_a_new_window() {
    let mut run = Run { agents: vec![agent("a")], calls: vec![call("a", 0, 0, 1_000_000, 0)], ..Default::default() };
    // Two agents' readings, unsorted and disagreeing; then the window resets at 9_000.
    run.readings = vec![
        reading("claude", 25.0, 5_000, 4_000),
        reading("claude", 18.0, 5_000, 1_000),
        reading("claude", 19.0, 5_000, 3_000),
        reading("claude", 20.0, 5_000, 2_000),
        reading("claude", 3.0, 9_000, 6_000),
        reading("claude", 5.0, 9_000, 7_000),
    ];
    let p = &plan::plan(&run)[0];
    let highs: Vec<f64> = p.points.iter().map(|x| x.2).collect();
    assert_eq!(highs, [18.0, 20.0, 20.0, 25.0, 3.0, 5.0]);
    assert_eq!(p.added.value, Some(12.0), "7 points in the first window and 5 in the second");
    assert_eq!(p.points_per_million.value, Some(12.0));
}

#[test]
fn an_identical_start_written_by_every_agent_could_have_been_written_once() {
    let mut run = Run {
        agents: vec![agent("a"), agent("b")],
        calls: vec![call("a", 0, 0, 1000, 0), call("b", 0, 0, 1000, 0)],
        ..Default::default()
    };
    for a in ["a", "b"] {
        run.items.push(ContextItem { fingerprint: Some(7), ..item(a, 0, ItemKind::Setup, 600) });
        run.items.push(ContextItem {
            fingerprint: Some(a.len() as u64 + u64::from(a.as_bytes()[0])),
            ..item(a, 0, ItemKind::Job, 400)
        });
    }
    let s = sharing::shared_start(&run, &tokens::size(&run));
    assert_eq!((s.agents, s.parts.len(), s.identical_per_agent.value), (2, 1, Some(600)));
    assert_eq!((s.first_call_cache_writes, s.writes_avoidable.value), (2000, Some(600)));
}

#[test]
fn tools_given_but_never_called_show_what_their_definitions_cost() {
    let mut run = Run {
        agents: vec![agent("a")],
        calls: vec![call("a", 0, 0, 1000, 0), call("a", 1, 1, 1100, 0)],
        ..Default::default()
    };
    run.items = vec![item("a", 0, ItemKind::ToolDefinitions, 1000), result("a", 1, "Read", "x.md", 10)];
    run.tools = vec![
        ToolDef { agent: "a".into(), name: "Read".into(), chars: 400 },
        ToolDef { agent: "a".into(), name: "Workflow".into(), chars: 600 },
    ];
    let t = sharing::tools(&run, &tokens::size(&run));
    let wf = t.iter().find(|x| x.tool == "Workflow").unwrap();
    assert_eq!((wf.calls, wf.definition_tokens.value, wf.definition_reread.value), (0, Some(600), Some(1200)));
    assert_eq!(t.iter().find(|x| x.tool == "Read").unwrap().calls, 1);
}

#[test]
fn agents_starting_close_together_are_a_fan_out_from_the_agent_that_handed_them_work() {
    let ids = ["lead", "d1", "d2", "d3", "d4", "d5"];
    let mut run = Run { agents: ids.iter().map(|i| agent(i)).collect(), ..Default::default() };
    run.calls = ids
        .iter()
        .enumerate()
        .map(|(k, i)| call(i, 0, if k == 0 { 0 } else { 600_000 + k as i64 * 10_000 }, 10, 1))
        .collect();
    run.edges = ids[1..]
        .iter()
        .map(|i| Edge { from: "lead".into(), to: (*i).into(), kind: EdgeKind::HandsWorkTo, at_ms: Some(590_000) })
        .collect();
    let t = time::timeline(&run);
    assert_eq!(t.fanouts.len(), 1);
    assert_eq!((t.fanouts[0].agents, t.fanouts[0].from.as_deref()), (5, Some("lead")));
}

#[test]
fn latency_splits_active_time_into_model_tools_and_waiting() {
    let timed = |seq, s, e| Call { started_ms: Some(s), ended_ms: Some(e), ..call("a", seq, e, 10, 1) };
    let mut run =
        Run { agents: vec![agent("a")], calls: vec![timed(0, 0, 2_000), timed(1, 5_000, 9_000)], ..Default::default() };
    run.items = vec![ContextItem { took_ms: Some(1_000), ..result("a", 1, "Bash", "ls", 5) }];
    let times = time::agent_times(&run);
    let l = time::latency(&run, &times);
    assert_eq!((l.model.median_ms, l.model.total_ms), (Some(4_000), 6_000));
    assert_eq!(
        (l.model_share.value, l.tool_share.value, l.waiting_share.value),
        (Some(6.0 / 9.0), Some(1.0 / 9.0), Some(2.0 / 9.0))
    );
    assert_eq!(l.tools["Bash"].count, 1);
}

#[test]
fn calls_without_times_leave_latency_unknown_not_zero() {
    let run = Run { agents: vec![agent("a")], calls: vec![call("a", 0, 0, 10, 1)], ..Default::default() };
    let l = time::latency(&run, &time::agent_times(&run));
    assert_eq!((l.model.count, l.model_share.provenance), (0, Provenance::Unknown));
}

fn arbitrary_run() -> impl Strategy<Value = Run> {
    let call = (0u8..4, 0i64..100_000, 0u64..5_000, 0u64..50_000, 0u64..5_000, 0u64..2_000);
    proptest::collection::vec(call, 1..40).prop_map(|cs| {
        let mut run = Run { agents: (0..4).map(|i| agent(&format!("a{i}"))).collect(), ..Default::default() };
        let mut seq = [0u32; 4];
        for (a, at, input, read, write, output) in cs {
            let k = usize::from(a);
            run.calls.push(Call {
                agent: format!("a{a}"),
                seq: seq[k],
                at_ms: at,
                model: None,
                usage: Usage { input, cache_read: read, cache_write: write, output },
                started_ms: None,
                ended_ms: None,
            });
            run.items.push(result(&format!("a{a}"), seq[k], "Read", "f.md", input + 1));
            seq[k] += 1;
        }
        run
    })
}

proptest! {
    #[test]
    fn every_total_equals_the_sum_of_the_calls(run in arbitrary_run()) {
        let r = analyze(&run);
        let sum: u64 = run.calls.iter().map(|c| c.usage.total()).sum();
        prop_assert_eq!(r.run.tokens, sum);
        prop_assert_eq!(r.agents.iter().map(|a| a.tokens).sum::<u64>(), sum);
        prop_assert_eq!(r.timeline.points.iter().map(|p| p.tokens).sum::<u64>(), sum);
        let output: u64 = run.calls.iter().map(|c| c.usage.output).sum();
        prop_assert_eq!(r.where_tokens_went.iter().find(|p| p.group == "output").unwrap().tokens, output);
    }
}

#[test]
fn a_fan_out_finding_names_the_agent_not_its_id() {
    let ids = ["lead", "d1", "d2", "d3", "d4", "d5"];
    let mut run = Run { agents: ids.iter().map(|i| agent(i)).collect(), ..Default::default() };
    run.agents[0].name = "Integration Manager".into();
    run.calls = ids.iter().enumerate().map(|(k, i)| call(i, 0, k as i64 * 10_000, 10, 1)).collect();
    run.edges = ids[1..]
        .iter()
        .map(|i| Edge { from: "lead".into(), to: (*i).into(), kind: EdgeKind::HandsWorkTo, at_ms: Some(1) })
        .collect();
    let r = analyze(&run);
    assert!(
        r.findings.iter().any(|f| f.text.contains("after Integration Manager handed out work")),
        "{:?}",
        r.findings
    );
}
