# 013 TUI Wizards and Resolution

Status: complete
Updated: 2026-07-24
Roadmap: `g02.004`
Spec: `docs/specs/004-tui-rebuild.md`

## Objective

Wizard framework with the UX-spec wart fixes (back-step, review, structured
options) and the superposition resolution view.

## In Scope

- wizard framework: ordered fields with defaults, back-one-step, final
  review screen before execution, structured choice prompts; unrecognized
  option input is an error, never swallowed
- login and publish wizards assembling argv for the CLI layer
- resolution view: list superposition paths with variant counts (from
  `resolve list`), assign per-path decisions, live validation counts, apply
  via `resolve apply` with a decisions file the TUI writes
- reducer tests: wizard back/review flow, invalid option rejection,
  decision assignment

## Out Of Scope

- agent trace (4.4)

## Acceptance Criteria

- wizards drive login/publish end to end through the CLI layer
- resolution flow works against a superposed snap in tests (reducer level)
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- needs CLI semantics that do not exist — extend CLI first

## Outcome

- wizard framework (`wizard.rs`): ordered fields with defaults, optional
  fields, structured choice prompts with unknown/ambiguous rejection (wart
  fix), Esc back-one-step restoring prior values, review screen before
  execution (wart fix); wizards emit argv only
- login + publish wizards; console `login`/`publish` and the remote primary
  action open them; publish gate defaults from remote config
- resolution view: `resolve <snap>` lists superposed paths with variant
  counts, 1-9/0 assign/clear decisions, Enter jumps to next undecided then
  applies via a TUI-written decisions file through `resolve apply`;
  undecided counter live in the view
- 6 wizard unit tests + reducer coverage; 39 workspace tests
- `effigy validate` green

## Next Task

Execute the Batch 4.4 agent-trace card (`014-tui-agent-trace.md`).
