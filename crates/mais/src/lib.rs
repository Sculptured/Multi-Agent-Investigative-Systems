//! MAIS, Multi-Agent Investigative Systems: reads what a multi-agent run's CLIs and orchestrator
//! recorded, ties every model call to its agent and that agent's place in the team, and explains
//! where the run's tokens and context went.
pub mod adapters;
pub mod analyze;
pub mod link;
pub mod locate;
pub mod model;
pub mod report;

use adapters::{sculptured::RunExport, team::Team};
use std::path::PathBuf;

/// The report for one run, from its session logs (files or folders), with the team and the run
/// export where there are.
pub fn report(
    logs: &[PathBuf],
    team: Option<Team>,
    export: Option<&RunExport>,
    sel: &link::Selection,
) -> Result<report::Report, String> {
    let (sessions, skipped) = link::read_logs(logs);
    if sessions.is_empty() {
        return Err("no Claude Code or Codex session logs found there".into());
    }
    let team = team.or_else(|| export.map(|x| x.team.clone()));
    let mut run = link::assemble(sessions, team.as_ref(), export, sel);
    run.skipped = skipped;
    Ok(analyze::analyze(&run))
}
