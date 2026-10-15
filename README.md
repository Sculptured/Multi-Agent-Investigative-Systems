# MAIS: Multi-Agent Investigative Systems

[![CI](https://github.com/Alexander-Aghili/Multi-Agent-Investigative-Systems/actions/workflows/ci.yml/badge.svg)](https://github.com/Alexander-Aghili/Multi-Agent-Investigative-Systems/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**Where did a multi-agent run's tokens and context go?**

MAIS reads the logs that Claude Code and Codex already write, links every model call to the agent
that made it and to that agent's place in a team, and explains the run's usage as causes you can
act on:

- the setup every agent loads before it does anything (skills lists, plugins, hooks, tool definitions);
- the files agents print into their own context, and the files several agents each read for themselves;
- the parts of every agent's start that were identical, but were written to the prompt cache once per agent;
- tools agents were given and never called;
- work handed out to many agents at once, and how many worked at the same time;
- the plan window (the 5-hour and 7-day limits) the run used, against its budget;
- how long agents spent waiting on the model, in tools, and waiting for work.

Every number carries its **provenance**: `measured` (the CLI reported it), `derived` (arithmetic on
measured numbers), `estimated` (a split by characters, or a price ratio) or `unknown` (shown as
unknown, never as zero).

MAIS produces data only: a JSON report with a published schema. Applications that read it, such as
[Sculptured](https://usesculptured.com), present it.

## Install

```sh
cargo install --git https://github.com/Alexander-Aghili/Multi-Agent-Investigative-Systems mais-cli
```

## Use

```sh
# The newest run under a Claude Code project folder
mais json ~/.claude/projects/-home-me-Projects-plane

# Codex sessions from one afternoon
mais json ~/.codex/sessions/2026/10/15 --since 2026-10-15T13:00:00-04:00 --until 2026-10-15T17:00:00-04:00

# Names, groups and the run budget from the team, sessions linked exactly by a Sculptured run export
mais json ~/.claude/projects/-home-me-Projects-plane --team plane.madl.yaml --run plane.sculptured-session.jsonl

# Leave out commands, file paths and message text before sharing a report
mais json … --redact --out report.json

# The report's JSON Schema
mais schema
```

Without `--since`, `--until` or `--run`, MAIS picks the newest run: the latest sessions that follow
one another with no gap of ten minutes.

## What it reads

| Source | What MAIS takes from it |
|---|---|
| Claude Code (`~/.claude/projects/<folder>/*.jsonl`, with `subagents/` and `tool-results/`) | usage per API message; tool calls and results; the request Claude Code sent (system prompt, tool definitions, start-up attachments); its own cost figure; compactions; timings |
| Codex (`~/.codex/sessions/YYYY/MM/DD/*.jsonl`) | usage per model response; tool calls and outputs; plan-window readings; compactions; timings |
| [MADL](https://github.com/Alexander-Aghili/Multi-Agent-Descriptor-Language) team files | agents, groups, connections, run budget |
| Sculptured run exports (`sculptured.session/v1`) | which session is which agent; handoffs; plan readings; budget wrap-up and stop |

Lines a reader does not know are counted and skipped, never fatal; the report lists how many each
file had.

## The report

`mais.report/v1` ([schema](schema/mais.report.v1.json)) has these sections:

| Section | What it holds |
|---|---|
| `run`, `agents` | totals by token kind, calls, models, starting and final context, per agent |
| `where_tokens_went` | every token by cause: the starting context re-read, what grew, and output |
| `costliest` | the items whose re-reads cost the most |
| `shared_reads` | files several agents read into their own context |
| `shared_start` | identical start parts across agents, and cache writes that could have been reads |
| `tools` | tools given, what their definitions cost, and how often they were called |
| `timeline` | tokens and agents working over time; fan-outs |
| `latency` | model call and tool durations; model, tool and waiting shares |
| `plan` | plan-window readings, the highest so far, points per million tokens, the budget |
| `cost` | the CLI's own figure, and the cost split by token kind |
| `findings` | plain sentences, each naming the section it rests on |

### How context is attributed

A CLI reports what each call read in total, not each part. MAIS sizes the parts:

1. The first call's context is split among the items it held by their share of characters (`estimated`).
2. A later call's rise in context is split among the items added just before it (`derived`); a rise
   with nothing to explain it is reported as growth the logs do not show.
3. An item is re-read by every later call until a compaction, which keeps the persistent start
   (system prompt, tool definitions, setup, memory) and replaces the rest with a summary.

## Repository

| Path | What it is |
|---|---|
| `crates/mais` | the library: readers (`adapters`), linking (`link`), the run model, analyses and the report |
| `crates/mais-cli` | the `mais` command |
| `schema/` | the report's JSON Schema, generated from the types |
| `fixtures/` | hand-written logs, team files and the golden report used by the tests |
| `docs/design/` | the design |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Report security problems privately: see [SECURITY.md](SECURITY.md).

## License

[Apache-2.0](LICENSE).
