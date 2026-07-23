# 011 TUI Command Layer and Shell Core

Status: complete
Updated: 2026-07-24
Roadmap: `g02.004`
Spec: `docs/specs/004-tui-rebuild.md`

## Objective

CLI-as-library plus the ratatui shell core: layout, console, palette, view
stack, and the local views — the UX spec's skeleton.

## In Scope

- `converge-cli` lib target: `execute(argv: &[String]) -> Result<Value>`
  returning the JSON envelope; binary main delegates to it
- `converge-tui`: five-region layout (header, view body, Last strip,
  suggestions, input+hints); command entry with live fuzzy suggestions and
  command history; view stack push/pop; layered Esc with explicit quit
  confirmation; Tab toggles Local/Remote context with the context named in
  the prompt
- local root view (pending changes, latest snap, sync state) and history
  view; Enter on empty input runs the state-computed primary action
  (changes -> snap, else history)
- smoke tests for the shell reducer (key events -> state transitions) —
  rendering tested by construction, not screenshots

## Out Of Scope

- remote dashboard, async runner (4.2), wizards/resolution (4.3), trace (4.4)

## Acceptance Criteria

- `converge` binary behavior unchanged (tests still green)
- TUI builds and runs; reducer tests cover Esc layering, Tab context,
  suggestion acceptance, primary-action selection
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- shell needs a semantic the CLI cannot express — stop, extend CLI first

## Outcome

- `converge-cli` is now lib + thin bin: `execute(argv) -> JSON value` runs
  the exact binary code path (`OutputMode::Capture`); added `changes` verb
  (working tree vs latest snap) the shell needed — extended CLI first per
  stop-condition rule
- `converge-tui` shell core: five-region layout, command console with live
  suggestions + command history, view stack, layered Esc with explicit quit
  confirm (wart fix), Tab context toggle with the context named in prompt
  and header (wart fix), Enter runs the state-computed primary action
  (changes -> snap, else history)
- local root + history views pull data through the CLI layer only
- 5 reducer unit tests: Esc layering, quit confirm, Tab semantics, primary
  action, argv submission, suggestion navigation
- `effigy validate` green: fmt, clippy -D warnings, 31 nextest tests

## Next Task

Execute the Batch 4.2 remote-views-and-async card
(`012-tui-remote-views-and-async.md`).
