# 012 TUI Remote Views and Async

Status: ready
Updated: 2026-07-24
Roadmap: `g02.004`
Spec: `docs/specs/004-tui-rebuild.md`

## Objective

Remote dashboard view and the async command runner: the event loop never
blocks on the network (UX spec wart 1).

## In Scope

- async runner: remote commands (`publish`, `fetch`, `status`, `login`)
  run on a worker thread; results return over a channel; the Last strip
  shows an in-flight marker until completion; local commands stay inline
- remote root view: configured remote target, last bundle status,
  recommended next action derived from state
- reducer/runtime tests: in-flight state, result delivery, local commands
  unaffected

## Out Of Scope

- wizards, resolution view (4.3), agent trace (4.4)

## Acceptance Criteria

- a slow remote command leaves the UI responsive (typing works mid-flight)
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- needs CLI semantics that do not exist — extend CLI first

## Next Task

On completion, open the Batch 4.3 wizards-and-resolution card.
