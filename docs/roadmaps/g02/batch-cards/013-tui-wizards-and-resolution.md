# 013 TUI Wizards and Resolution

Status: ready
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

## Next Task

On completion, open the Batch 4.4 agent-trace card.
