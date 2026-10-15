# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Readers for Claude Code logs (with subagents, stored tool results, request shape, cost and
  compactions) and Codex logs (with plan-window readings and compactions).
- Linking of sessions to team members from a Sculptured run export or the role line in an agent's
  first message; teams and run budgets from MADL files and run exports.
- Analyses: where tokens went by cause, costliest items, shared reads, shared start and avoidable
  cache writes, tool use, timeline and fan-outs, latency, plan windows and cost by token kind.
- The `mais.report/v1` JSON report and its schema; `mais json` and `mais schema`; `--redact`.
