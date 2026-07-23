# 014 TUI Agent Trace

Status: complete
Updated: 2026-07-24
Roadmap: `g02.004`
Spec: `docs/specs/004-tui-rebuild.md`

## Objective

JSONL semantic trace making the TUI machine-drivable and observable
(UX spec §4.3): screen views with selectable items and primary CTA, user
actions, classified errors, session lifecycle.

## In Scope

- `--agent-trace <path>` flag (and `CONVERGE_AGENT_TRACE` env): append-only
  JSONL
- events: `session_start`, `screen_view` (screen id, view title, selectable
  items, primary CTA — deduped by signature so the trace records semantic
  transitions, not frames), `user_action` (key or command, canonical form),
  `command_result` (ok or classified error), `session_end` with counts
- unit tests: signature dedup, event shapes, error classification

## Out Of Scope

- new views or verbs

## Acceptance Criteria

- running the TUI with tracing produces valid JSONL covering a session
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- none specific

## Outcome

- `trace.rs`: JSONL sink via `--agent-trace` / `CONVERGE_AGENT_TRACE`;
  events session_start / screen_view (screen id, selectable items, primary
  CTA; deduped by signature) / user_action (canonical + raw) /
  command_result (ok or validation-vs-system classified error) /
  session_end with counters
- runtime emits screen views per semantic transition and traces every
  action and command result, including async worker results
- bonus wart-2 closure: Alt+h / Alt+r contextual jump keys (Alt+N template)
- 3 trace tests + jump-key reducer test; 43 workspace tests
- `effigy validate` green

## Next Task

Close roadmap `g02.004`; ask operator intent for the next owner.
