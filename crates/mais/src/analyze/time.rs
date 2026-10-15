//! When the tokens went (timeline, fan-outs) and how long things took (latency).
use crate::model::{AgentId, EdgeKind, Figure, ItemKind, Run};
use crate::report::{Durations, Fanout, Latency, TimePoint, Timeline};
use std::collections::{BTreeMap, BTreeSet};

/// Agents starting within this long of each other count as one fan-out…
const FANOUT_WINDOW_MS: i64 = 2 * 60 * 1000;
/// …when there are at least this many.
const FANOUT_AGENTS: usize = 5;

/// An interval of about a sixtieth of the run, in whole 10 seconds (at least 10 s).
fn interval(span_ms: i64) -> i64 {
    ((span_ms / 60).max(10_000) / 10_000) * 10_000
}

pub fn timeline(run: &Run) -> Timeline {
    let Some(start) = run.calls.iter().map(|c| c.at_ms).min() else { return Timeline { interval_ms: 10_000, points: vec![], fanouts: vec![] } };
    let end = run.calls.iter().map(|c| c.at_ms).max().unwrap_or(start);
    let step = interval(end - start);
    let mut buckets: BTreeMap<i64, (u64, BTreeSet<&str>)> = BTreeMap::new();
    for c in &run.calls {
        let b = buckets.entry(start + (c.at_ms - start) / step * step).or_default();
        b.0 += c.usage.total();
        b.1.insert(&c.agent);
    }
    let points = buckets.into_iter().map(|(at_ms, (tokens, a))| TimePoint { at_ms, tokens, agents_working: a.len() as u32 }).collect();
    Timeline { interval_ms: step, points, fanouts: fanouts(run) }
}

/// Groups of agents whose first calls came close together.
fn fanouts(run: &Run) -> Vec<Fanout> {
    let mut firsts: Vec<(i64, &str)> = run.agents.iter().filter_map(|a| Some((run.calls_of(&a.id).map(|c| c.at_ms).min()?, a.id.as_str()))).collect();
    firsts.sort();
    let mut out = Vec::new();
    let mut i = 0;
    while i < firsts.len() {
        let group: Vec<&(i64, &str)> = firsts[i..].iter().take_while(|(t, _)| *t - firsts[i].0 <= FANOUT_WINDOW_MS).collect();
        if group.len() >= FANOUT_AGENTS {
            out.push(Fanout { at_ms: firsts[i].0, agents: group.len() as u32, from: handed_by(run, &group) });
            i += group.len();
        } else {
            i += 1;
        }
    }
    out
}

/// The agent that handed work to most of `group`, if the run says.
fn handed_by(run: &Run, group: &[&(i64, &str)]) -> Option<AgentId> {
    let ids: BTreeSet<&str> = group.iter().map(|(_, a)| *a).collect();
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for e in run.edges.iter().filter(|e| e.kind == EdgeKind::HandsWorkTo && e.at_ms.is_some() && ids.contains(e.to.as_str())) {
        *count.entry(&e.from).or_default() += 1;
    }
    count.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a.to_string())
}

pub fn durations(mut v: Vec<i64>) -> Durations {
    v.sort_unstable();
    let at = |q: f64| v.get(((v.len() as f64 - 1.0) * q).round() as usize).copied();
    Durations { count: v.len() as u32, total_ms: v.iter().sum(), median_ms: at(0.5), p90_ms: at(0.9) }
}

/// Per agent: (model, tools, waiting) time.
#[derive(Clone, Debug, Default)]
pub struct Times {
    pub model: Figure<i64>,
    pub tools: Figure<i64>,
    pub waiting: Figure<i64>,
}
pub type AgentTimes = BTreeMap<AgentId, Times>;

fn call_ms(c: &crate::model::Call) -> Option<i64> {
    Some(c.ended_ms? - c.started_ms?).filter(|d| *d >= 0)
}

pub fn agent_times(run: &Run) -> AgentTimes {
    run.agents.iter().map(|a| {
        let calls: Vec<_> = run.calls_of(&a.id).collect();
        let timed: Vec<i64> = calls.iter().filter_map(|c| call_ms(c)).collect();
        let model = (timed.len() == calls.len() && !calls.is_empty()).then(|| timed.iter().sum::<i64>());
        let tools: i64 = run.items.iter().filter(|i| i.agent == a.id && i.kind == ItemKind::ToolResult).filter_map(|i| i.took_ms).sum();
        let span = calls.iter().map(|c| c.ended_ms.unwrap_or(c.at_ms)).max().zip(calls.iter().map(|c| c.started_ms.unwrap_or(c.at_ms)).min()).map(|(e, s)| e - s);
        let waiting = model.zip(span).map(|(m, s)| (s - m - tools).max(0));
        let t = Times {
            model: model.map_or(Figure::unknown(), Figure::measured),
            tools: Figure::derived(tools),
            waiting: waiting.map_or(Figure::unknown(), Figure::derived),
        };
        (a.id.clone(), t)
    }).collect()
}

pub fn latency(run: &Run, times: &AgentTimes) -> Latency {
    let model = durations(run.calls.iter().filter_map(call_ms).collect());
    let mut by_model: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for c in &run.calls {
        if let (Some(m), Some(d)) = (&c.model, call_ms(c)) {
            by_model.entry(m.clone()).or_default().push(d);
        }
    }
    let mut tools: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for i in run.items.iter().filter(|i| i.kind == ItemKind::ToolResult) {
        if let (Some(t), Some(d)) = (&i.tool, i.took_ms) {
            tools.entry(t.clone()).or_default().push(d);
        }
    }
    let sum = |f: fn(&Times) -> Option<i64>| times.values().map(f).sum::<Option<i64>>();
    let (m, t, w) = (sum(|x| x.model.value), sum(|x| x.tools.value), sum(|x| x.waiting.value));
    let all = m.zip(t).zip(w).map(|((m, t), w)| (m + t + w) as f64).filter(|a| *a > 0.0);
    let share = |x: Option<i64>| x.zip(all).map_or(Figure::unknown(), |(x, a)| Figure::derived(x as f64 / a));
    Latency {
        model,
        by_model: by_model.into_iter().map(|(k, v)| (k, durations(v))).collect(),
        tools: tools.into_iter().map(|(k, v)| (k, durations(v))).collect(),
        model_share: share(m), tool_share: share(t), waiting_share: share(w),
    }
}
