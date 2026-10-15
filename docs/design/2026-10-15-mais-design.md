# MAIS: Multi-Agent Investigative Systems

Status: draft for review, 15 October 2026.

## Purpose

MAIS answers one question about a run of a multi-agent system: **where did the tokens, and the
context, go?** It reads what the agents' CLIs and their orchestrator already record, ties every
model call to the agent that made it and to that agent's place in the team, and explains the run's
usage as causes a person can act on: the setup every agent loads, the files they print, the work
handed out at once, the plan window they use up.

It started as a one-off investigation of a 26-agent run that used 30 points of a Claude 5-hour
window in ten minutes. That investigation found causes no existing tool shows (the person's own
Claude Code setup was 40% of every agent's starting context; printing whole files was 29% of all
tokens). MAIS makes that investigation repeatable for any run.

**Who it is for:** anyone running several Claude Code or Codex agents together, with or without
Sculptured. Sculptured uses it for its own **Where tokens went** view.

**Success:** given the session logs of the 15 October C172 run and its MADL team file, `mais report`
reproduces that investigation's findings: the same totals within 1%, the same top causes in the same
order, and every figure labelled as measured, estimated or unknown.

## Decisions

| Question | Decision |
|---|---|
| Fork an observability platform? | No. None covers context composition, re-read cost or plan windows; the platforms need ClickHouse and several services; licences (ELv2, AGPL) or maintenance status rule others out. MAIS exports to them later instead. |
| Language and licence | Rust; Apache-2.0, like MADL and Sculptured's open core. |
| Team structure | From a MADL document. Without one, each session is an agent with no team. |
| First version | Claude Code and Codex logs, Sculptured run exports, MADL; JSON and HTML reports. |
| Second version | OpenTelemetry GenAI spans out (for Langfuse, Phoenix, Opik, Grafana); Claude Code's own OpenTelemetry export in. |
| Where Sculptured runs it | On the runner, on the computer that holds the logs. Only the report leaves it. |

## What it reads

| Source | What MAIS takes from it |
|---|---|
| Claude Code session logs (`~/.claude/projects/<folder>/<session>.jsonl`, with `<session>/subagents/` and `<session>/tool-results/`) | each API message's usage (counted once per `message.id`); tool calls and results joined by `tool_use_id`, following results stored in `tool-results/`; the request Claude Code sent (`api-request-shape`, `api-request-blob`: system prompt, tool definitions, start-up attachments such as the skills list); Claude Code's own totals and costs (`cost-state`); compaction; model |
| Codex session logs (`~/.codex/sessions/YYYY/MM/DD/*.jsonl`) | `token_count` usage per model call; tool calls and outputs; `rate_limits` readings (percent, window, reset time); compaction; `session_meta` (folder, id) |
| Sculptured run export (`sculptured.session/v1`) | which session is which agent; handoffs and spawned children; plan readings; run budget, wrap-up and stop; the run's start and end |
| MADL document | agents, groups, connections, roles, models, budgets |

Each reader is an adapter behind one trait, so a new CLI (Gemini CLI, OpenCode) is one more adapter.
Unknown lines are counted and skipped, never fatal; the report lists how many each file had.

**Linking sessions to agents**, in order: the Sculptured run export's session ids; a MADL agent named
in the session's first message (Sculptured writes "Your role: …"); one agent per session, named by
its folder and start time.

## The model

```text
Run ── agents ── calls ── context items
 │        │         └── tool results (with the call that asked and the call that first re-sent it)
 │        └── edges: hands work to, spawned, in group
 ├── plan readings (provider, window, percent, resets at, time, reporter)
 └── budget events (wrap-up, stop, limits)
```

- **Call**: agent, sequence, time, model; input, cache read, cache write, output tokens; total context
  (input + cache read + cache write).
- **Context item**: something in a call's context, with the call that added it, its size and its kind:
  `system-prompt`, `tool-definitions`, `setup` (skills list, plugin and MCP instructions, deferred
  tool names, hooks), `memory` (CLAUDE.md and the like), `job` (the first message), `tool-result`
  (with tool name and the command, file or query), `message` (from a person or teammate), `reply`
  (the agent's own output). Items start over after compaction.
- Every number carries a **provenance**: `measured` (the CLI reported it), `derived` (arithmetic on
  measured numbers), `estimated` (a split by share of characters, or a price) or `unknown`.

## What it works out

1. **Totals** per run, agent, group, model and kind of token. They must equal the sum of the calls.
2. **Composition** of each agent's starting context, from the request Claude Code sent. Sizes are
   tokens where the request carries them and otherwise the item's share of characters, scaled to the
   measured first-call context and marked `estimated`.
3. **Re-read cost**: an item added before call *k* is re-sent on every later call until compaction:
   cost = its tokens × the calls that re-send it. Ranked across the run by tool, command, file and
   agent.
4. **Growth attribution** when an item's own size is unknown: the rise in context between two calls,
   shared among the tool results in between by their size in characters (`derived`).
5. **Shared reads**: files and commands several agents read for themselves, and what those copies cost.
6. **Timeline**: tokens per interval, agents working at once, handoffs and fan-outs marked.
7. **Plan window**: readings over time (and the highest so far, since readings from different agents
   disagree by a few points); the run's share against its budget's soft and hard values, wrap-up and
   stop; percentage points per million tokens.
8. **Cost** at API prices: the CLI's own figure where it gives one (Claude Code); otherwise a dated
   price table, marked `estimated`.
9. **Findings**: plain sentences built from the analyses, each pointing to its numbers, for example
   "Your Claude Code setup is 40% (estimated) of every agent's starting context".

## What it writes

- **`mais report <logs…> [--team team.madl] [--run run.sculptured-session.jsonl] --out report.html`**:
  one self-contained HTML file. No network and no server; it opens in a browser. It shows the figures,
  charts and tables of the C172 investigation, built from the JSON below.
- **`mais json …`**: the report as JSON (`mais.report/v1`), with a published JSON Schema. Sculptured
  and other tools read this.
- **Selection**: `--since`, `--until`, `--folder`, `--session`; the default is the newest run found
  under the paths given.
- **Privacy**: everything stays on the computer. `--redact` removes commands, file paths and message
  text from the report, leaving sizes and kinds.

## Repository

Laid out like MADL: a library crate `mais` (adapters, model, analyses, report schema) and a
`mais-cli` crate; `fixtures/` with anonymised logs made by fixture CLIs and by hand (no real model
calls); `docs/` (concepts, the report schema, each analysis's definition); README, LICENSE
(Apache-2.0), CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CHANGELOG, CI.

## Inside Sculptured

- **On the runner**: a local operation that runs MAIS over the logs of a team's run, on the computer
  that ran it, with the run's export and MADL from controld. It returns the JSON report; prompts,
  files and command output never leave the computer (commands and file names do, unless redacted).
- **In controld**: the report is stored per run, refreshed every minute while the run goes and once
  after it ends.
- **On the board**: a **Where tokens went** view on the Run board and in History: the headline
  figures, plan window against the budget, tokens over time with agents working, where the tokens
  went, tokens by agent, the costliest tool results, files many agents read, and the findings. Each
  figure links to the agent, call or tool result behind it.
- Sculptured pins MAIS by git revision, as it does MADL.

## Testing

- Adapters: fixture logs for each CLI, including messages split over several lines, results stored
  in `tool-results/`, subagents, compaction and resumed sessions; unknown lines are skipped.
- Analyses: unit tests on small built runs (re-read cost, compaction reset, growth attribution,
  plan-window highest-so-far, window reset).
- Report: golden JSON for the fixture runs; totals equal the sum of calls (a property test).
- The C172 acceptance check runs against a local copy of those logs, outside CI (they are private).
- Sculptured: the runner operation on fixture-CLI runs; the view renders the report (web tests).

## Open points

- Tokenising locally for exact composition needs each model's tokenizer; the first version uses
  character shares scaled to measured totals and says so.
- Claude Code's log format is undocumented and changes; adapters are tested on recorded samples and
  report unknown shapes instead of failing.
