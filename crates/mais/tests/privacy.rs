//! What a report may carry: command lines and file names, with secrets masked; never file contents.
use mais::adapters::{codex, scrub};
use std::io::Write;

#[test]
fn secrets_in_command_lines_are_masked() {
    let cases = [
        (
            "curl -H 'Authorization: Bearer abc.def-123' https://x.io",
            "curl -H 'Authorization: Bearer ***' https://x.io",
        ),
        ("export OPENAI_API_KEY=sk-live-1234567890abcdef && run", "export OPENAI_API_KEY=*** && run"),
        ("psql postgres://admin:hunter2@db:5432/app", "psql postgres://***@db:5432/app"),
        ("tool --password s3cret --token=t0k", "tool --password *** --token=***"),
        ("echo ghp_abcdefghijklmnopqrstuvwxyz0123", "echo ***"),
        ("sed -n 1,40p plans/PLAN.md", "sed -n 1,40p plans/PLAN.md"),
    ];
    for (raw, want) in cases {
        assert_eq!(scrub(raw), want, "{raw}");
    }
}

#[test]
fn a_codex_patch_is_named_by_its_files_never_its_text() {
    let dir = std::env::temp_dir().join(format!("mais-privacy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("rollout-patch.jsonl");
    let patch = "*** Begin Patch\n*** Update File: config/.env\n@@\n+DATABASE_URL=postgres://admin:hunter2@db/app\n*** Add File: src/new.rs\n+fn main() {}\n*** End Patch";
    let lines = [
        serde_json::json!({"timestamp":"2026-10-15T10:00:00.000Z","type":"session_meta","payload":{"id":"cx-9","cwd":"/w"}}),
        serde_json::json!({"timestamp":"2026-10-15T10:00:01.000Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"c1","input":patch}}),
        serde_json::json!({"timestamp":"2026-10-15T10:00:02.000Z","type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c1","output":"Success"}}),
        serde_json::json!({"timestamp":"2026-10-15T10:00:03.000Z","type":"token_usage_record","payload":{"response_id":"r1","usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":5}}}),
    ];
    let mut f = std::fs::File::create(&file).unwrap();
    for l in lines {
        writeln!(f, "{l}").unwrap();
    }
    let s = codex::read(&file).unwrap();
    let target = s.items.iter().find_map(|i| i.target.clone()).expect("the patch's result has a target");
    assert_eq!(target, "config/.env, src/new.rs");
}
