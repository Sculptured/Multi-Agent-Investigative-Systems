use mais::locate::{claude_session, codex_session};
use std::fs;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mais-locate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_claude_session_is_found_in_whichever_project_folder_holds_it() {
    let projects = temp("claude");
    fs::create_dir_all(projects.join("-home-me-a")).unwrap();
    fs::create_dir_all(projects.join("-home-me-b")).unwrap();
    fs::write(projects.join("-home-me-b/abc-123.jsonl"), "{}\n").unwrap();
    assert_eq!(claude_session(&projects, "abc-123"), Some(projects.join("-home-me-b/abc-123.jsonl")));
    assert_eq!(claude_session(&projects, "../abc-123"), None, "an id is a file name, never a path");
    assert_eq!(claude_session(&projects, "nope"), None);
}

#[test]
fn a_codex_session_is_found_by_id_in_the_day_folders_near_the_run() {
    let sessions = temp("codex");
    let day = sessions.join("2026/10/15");
    fs::create_dir_all(&day).unwrap();
    fs::create_dir_all(sessions.join("2026/09/01")).unwrap();
    fs::write(sessions.join("2026/09/01/rollout-2026-09-01T10-00-00-0199-old.jsonl"), "{}\n").unwrap();
    let file = day.join("rollout-2026-10-15T15-47-31-0199aa.jsonl");
    fs::write(&file, "{}\n").unwrap();
    let at = 1_792_093_651_000; // 2026-10-15T19:47:31Z
    assert_eq!(codex_session(&sessions, "0199aa", at), Some(file));
    assert_eq!(codex_session(&sessions, "0199-old", at), None, "a month before the run is not searched");
    assert_eq!(codex_session(&sessions, "a/b", at), None);
}
