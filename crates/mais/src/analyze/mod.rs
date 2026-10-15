//! The analyses: from a `Run` to a `Report`.
pub mod causes;
mod findings;
pub mod plan;
pub mod sharing;
pub mod summary;
pub mod time;
pub mod tokens;

use crate::model::Run;
use crate::report::{FORMAT, Report};

/// Rows kept in the long lists.
pub const TOP: usize = 25;

pub fn analyze(run: &Run) -> Report {
    let items = tokens::size(run);
    let timeline = time::timeline(run);
    let most = timeline.points.iter().map(|p| p.agents_working).max().unwrap_or(0);
    let times = time::agent_times(run);
    let mut r = Report {
        format: FORMAT.into(),
        run: summary::run_summary(run, most),
        agents: summary::agent_rows(run, &items, &times),
        where_tokens_went: causes::where_tokens_went(run, &items),
        costliest: causes::costliest(&items, TOP),
        shared_reads: causes::shared_reads(&items, TOP),
        shared_start: sharing::shared_start(run, &items),
        tools: sharing::tools(run, &items),
        latency: time::latency(run, &times),
        timeline,
        plan: plan::plan(run),
        cost: plan::cost(run),
        findings: vec![],
        skipped: run.skipped.clone(),
    };
    r.findings = findings::findings(&r);
    r
}
