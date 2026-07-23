# 012 TUI Remote Views and Async

Status: complete
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

## Outcome

- async worker: `publish`/`fetch`/`status`/`login` run off-thread via mpsc;
  event loop polls results non-blocking; Last strip shows a yellow
  "… <cmd> (running)" marker until delivery — typing stays live mid-flight
- new CLI verb `remote` (config + last published snap); `publish` now
  records last-published state
- remote root view: target `repo/scope/gate @ url`, last published snap,
  context-aware primary action (unconfigured -> login, else publish)
- reducer tests for remote-command classification and context-dependent
  primary action; workspace tests 33
- `effigy validate` green

## Next Task

Execute the Batch 4.3 wizards-and-resolution card
(`013-tui-wizards-and-resolution.md`).
