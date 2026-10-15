use mais::adapters::team;
use mais::link::{assemble, read_logs, Selection};
use mais::report::{redact, schema, Report};
use std::path::PathBuf;

fn root(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

/// `actual` must equal the checked-in file; `UPDATE_SCHEMAS=1` rewrites it.
fn matches_file(path: &str, actual: &str) {
    let p = root(path);
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::write(&p, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&p).unwrap_or_default();
    assert!(expected == actual, "{path} is out of date: run UPDATE_SCHEMAS=1 cargo test");
}

fn fixture_report() -> Report {
    let t = team::from_madl(&madl::load(&root("fixtures/team/c172.madl.yaml")).unwrap());
    let (sessions, skipped) = read_logs(&[root("fixtures/claude/proj")]);
    let mut run = assemble(sessions, Some(&t), None, &Selection::default());
    run.skipped = skipped.into_iter().map(|(f, n)| (f.rsplit('/').next().unwrap().to_string(), n)).collect();
    mais::analyze::analyze(&run)
}

#[test]
fn the_published_schema_matches_the_report_types() {
    matches_file("schema/mais.report.v1.json", &(serde_json::to_string_pretty(&schema()).unwrap() + "\n"));
}

#[test]
fn the_fixture_runs_report_is_unchanged() {
    matches_file("fixtures/golden/claude-team.report.json", &(serde_json::to_string_pretty(&fixture_report()).unwrap() + "\n"));
}

#[test]
fn a_report_reads_back_as_the_same_report() {
    let r = fixture_report();
    assert_eq!(serde_json::from_str::<Report>(&serde_json::to_string(&r).unwrap()).unwrap(), r);
}

#[test]
fn a_redacted_report_keeps_sizes_but_no_commands_paths_or_file_names() {
    let r = fixture_report();
    let targets: Vec<String> = r.costliest.iter().filter_map(|c| c.target.clone()).chain(r.shared_reads.iter().map(|s| s.target.clone())).chain(r.skipped.keys().cloned()).collect();
    assert!(!targets.is_empty(), "the fixture has targets to hide");
    let mut red = r.clone();
    redact(&mut red);
    let json = serde_json::to_string(&red).unwrap();
    for t in &targets {
        assert!(!json.contains(&serde_json::to_string(t).unwrap()), "{t} is still in the report");
    }
    assert_eq!(red.run.tokens, r.run.tokens);
    assert_eq!(red.costliest.iter().map(|c| c.reread.value).collect::<Vec<_>>(), r.costliest.iter().map(|c| c.reread.value).collect::<Vec<_>>());
}
