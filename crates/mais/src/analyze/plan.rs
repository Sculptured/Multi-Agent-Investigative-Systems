//! The plan windows the run used, against its budget, and what it cost.
use crate::model::{Figure, Provenance, Run, Runtime};
use crate::report::{Cost, CostKind, PlanSeries};
use std::collections::BTreeMap;

pub fn plan(run: &Run) -> Vec<PlanSeries> {
    let mut by: BTreeMap<(String, String), Vec<&crate::model::PlanReading>> = BTreeMap::new();
    for r in &run.readings {
        by.entry((r.provider.clone(), r.window.clone())).or_default().push(r);
    }
    by.into_iter().map(|((provider, window), rs)| one(run, provider, window, rs)).collect()
}

fn one(run: &Run, provider: String, window: String, mut rs: Vec<&crate::model::PlanReading>) -> PlanSeries {
    rs.sort_by_key(|r| r.at_ms);
    let (points, added) = series(&rs);
    let mine = |c: &&crate::model::Call| runtime_provider(run, &c.agent) == Some(provider.as_str());
    let tokens: u64 = run.calls.iter().filter(mine).map(|c| c.usage.total()).sum();
    let per_m = (tokens > 0).then(|| added / (tokens as f64 / 1e6));
    let limits = run.budget.as_ref().map(|b| b.plan.as_slice()).unwrap_or_default();
    let budget = limits.iter().find(|(p, w, _, _)| *p == provider && *w == window).map(|(_, _, s, h)| (*s, *h));
    PlanSeries {
        first: points.first().map_or(0.0, |p| p.1),
        highest: points.iter().map(|p| p.2).fold(0.0, f64::max),
        points,
        provider,
        window,
        added: Figure::derived(added),
        points_per_million: per_m
            .map_or(Figure::unknown(), |v| Figure { value: Some(v), provenance: Provenance::Derived }),
        budget,
        events: run.budget_events.clone(),
    }
}

/// (time, reading, highest so far) through each window; readings from different agents disagree a
/// little, so the highest so far is the series. A later reset time starts a new window. Returns the
/// points and the points added across windows.
fn series(rs: &[&crate::model::PlanReading]) -> (Vec<(i64, f64, f64)>, f64) {
    let (mut out, mut added) = (Vec::new(), 0.0);
    let (mut base, mut high, mut resets): (Option<f64>, f64, Option<i64>) = (None, 0.0, None);
    for r in rs {
        if resets.zip(r.resets_at).is_some_and(|(was, now)| now > was) {
            added += high - base.unwrap_or(high);
            base = Some(0.0);
            high = 0.0;
        }
        resets = r.resets_at.or(resets);
        base.get_or_insert(r.percent);
        high = high.max(r.percent);
        out.push((r.at_ms, r.percent, high));
    }
    (out, added + high - base.unwrap_or(high))
}

fn runtime_provider<'a>(run: &'a Run, agent: &str) -> Option<&'a str> {
    run.agent(agent).map(|a| match a.runtime {
        Runtime::ClaudeCode => "claude",
        Runtime::Codex => "codex",
    })
}

pub fn cost(run: &Run) -> Cost {
    let usd: Vec<f64> = run.agents.iter().filter(|a| a.parent.is_none()).filter_map(|a| a.cli_cost_usd).collect();
    let all = run.agents.iter().filter(|a| a.parent.is_none()).count();
    let cli_usd = if usd.is_empty() {
        Figure::unknown()
    } else if usd.len() == all {
        Figure::measured(usd.iter().sum())
    } else {
        Figure::estimated(usd.iter().sum())
    };
    let u: crate::model::Usage = run.calls.iter().map(|c| c.usage).sum();
    let kinds = [
        ("Re-reading the cache", u.cache_read, 0.1),
        ("Writing to the cache", u.cache_write, 1.25),
        ("Output and thinking", u.output, 5.0),
        ("Fresh input", u.input, 1.0),
    ];
    let total: f64 = kinds.iter().map(|(_, t, w)| *t as f64 * w).sum();
    let by_kind = kinds
        .iter()
        .map(|(k, t, w)| CostKind {
            kind: k.to_string(),
            tokens: *t,
            weight: *w,
            share: if total > 0.0 { *t as f64 * w / total } else { 0.0 },
        })
        .collect();
    Cost { cli_usd, by_kind }
}
