use clap::{Args, Parser, Subcommand};
use mais::adapters::{sculptured, team};
use mais::link::{Selection, assemble, read_logs};
use std::path::PathBuf;
use std::process::ExitCode;

/// Where a multi-agent run's tokens and context went.
#[derive(Parser)]
#[command(name = "mais", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The report as JSON (`mais.report/v1`).
    Json(Input),
    /// The JSON Schema of the report.
    Schema,
}

#[derive(Args)]
struct Input {
    /// Session logs: files, or folders searched for `*.jsonl` (Claude Code and Codex).
    #[arg(required = true)]
    logs: Vec<PathBuf>,
    /// The team, as a MADL file: names agents, groups them, gives the run budget.
    #[arg(long)]
    team: Option<PathBuf>,
    /// A Sculptured run export (`.sculptured-session.jsonl`): links sessions to agents exactly.
    #[arg(long)]
    run: Option<PathBuf>,
    /// Only calls from this time on (RFC 3339, e.g. 2026-10-15T15:40:00-04:00).
    #[arg(long)]
    since: Option<String>,
    #[arg(long)]
    until: Option<String>,
    /// Only sessions whose id starts with this (repeatable).
    #[arg(long)]
    session: Vec<String>,
    /// Leave out commands, file paths and message text.
    #[arg(long)]
    redact: bool,
    /// Write here instead of standard output.
    #[arg(long)]
    out: Option<PathBuf>,
}

fn time(s: &Option<String>) -> Result<Option<i64>, String> {
    s.as_deref()
        .map(|t| mais::adapters::ms(t).ok_or_else(|| format!("{t} is not a time such as 2026-10-15T15:40:00-04:00")))
        .transpose()
}

fn report(i: &Input) -> Result<mais::report::Report, String> {
    let team = match &i.team {
        Some(p) => Some(team::from_madl(&madl::load(p).map_err(|e| format!("{}: {e:?}", p.display()))?)),
        None => None,
    };
    let export = match &i.run {
        Some(p) => Some(sculptured::read(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?)?),
        None => None,
    };
    let sel = Selection { since_ms: time(&i.since)?, until_ms: time(&i.until)?, sessions: i.session.clone() };
    let (sessions, skipped) = read_logs(&i.logs);
    if sessions.is_empty() {
        return Err("no Claude Code or Codex session logs found there".into());
    }
    let team = team.or_else(|| export.as_ref().map(|x| x.team.clone()));
    let mut run = assemble(sessions, team.as_ref(), export.as_ref(), &sel);
    run.skipped = skipped;
    let mut r = mais::analyze::analyze(&run);
    if i.redact {
        mais::report::redact(&mut r);
    }
    Ok(r)
}

fn write(i: &Input, text: &str) -> Result<(), String> {
    match &i.out {
        Some(p) => std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display())),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.cmd {
        Cmd::Json(i) => write(&i, &serde_json::to_string_pretty(&report(&i)?).expect("the report serializes")),
        Cmd::Schema => {
            println!("{}", serde_json::to_string_pretty(&mais::report::schema()).expect("the schema serializes"));
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mais: {e}");
            ExitCode::FAILURE
        }
    }
}
