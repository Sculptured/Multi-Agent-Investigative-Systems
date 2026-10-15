# Contributing to MAIS

Thank you for helping. Bug reports with the smallest log that shows the problem are the most useful
contribution: agent CLIs change their log formats often.

## Working on the code

```sh
cargo test --workspace                 # readers, analyses, schema and golden report
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
UPDATE_SCHEMAS=1 cargo test            # after changing report types or an intended change to the golden report; review the diff
```

- Functions stay at 30 lines or fewer, files under 300.
- Tests never call a model or a network service. Fixtures are hand-written; never commit real
  session logs, which hold people's prompts, code and file paths.
- Every number in the report carries its provenance. A value the logs do not give is `unknown`,
  never zero.
- A new CLI is a new adapter under `crates/mais/src/adapters/` that produces a `Session`, with
  fixtures and tests for each kind of line it reads.

## Changes to the report

The report format is `mais.report/v1`. Adding a field is a compatible change; renaming or removing
one, or changing what a value means, needs a new format version. Update the schema, the golden
report and `CHANGELOG.md` in the same pull request.

## Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
