## What this changes

## Checklist

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` pass
- [ ] A change to the report updates the schema and golden report (`UPDATE_SCHEMAS=1 cargo test`) and `CHANGELOG.md`
- [ ] A new kind of log line has a hand-written fixture and a test; no real session logs are committed
