# 008 CLI Verb Surface

Status: ready
Updated: 2026-07-23
Roadmap: `g02.003`
Spec: `docs/specs/003-rebuild-vertical-slice.md`

## Objective

Give `converge-cli` its canonical verb surface over `converge-client`: the
local verbs of the vertical slice, each with stable argv and `--json` output.

## In Scope

- clap-based `converge` binary: `init`, `snap [-m msg]`, `history`,
  `restore <snap>`, `diff <a> <b>`, `resolve` (list/validate/apply from a
  decisions file)
- `--json` mode on every verb: stable envelope (`ok`, `data`/`error`),
  suitable for TUI and agents (arch 15: CLI is the semantic contract)
- human output plain and terse; exit codes: 0 ok, 1 domain error, 2 usage
- integration tests driving the compiled binary (happy path + one failure
  path per verb)

## Out Of Scope

- remote verbs (publish/login/fetch — Batch 3.5 wires them to the server)
- TUI

## Acceptance Criteria

- all verbs work against a real workspace in tests, both output modes
- `--json` output round-trips through serde without free-text parsing
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- a verb needs client behavior that does not exist — stop and route through
  the roadmap rather than growing ad-hoc client API mid-batch

## Next Task

On completion, open the Batch 3.4 server-slice card.
