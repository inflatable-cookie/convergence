# 064 Reducer Suite And Spec Reconciliation

Status: complete
Updated: 2026-07-25
Roadmap: `g02.017`

## Objective

Close the roadmap: cover the reducer with tests, decide workflow
profiles explicitly, and reconcile `docs/rebuild/002` with what was
actually built — including the minor warts (missing cursor keys,
context boundary) the audit listed but no batch owned.

## Scope of the actual problem

The reducer is pure by design and had zero tests when the audit ran;
17.1-17.3 added coverage as they went, leaving the input line, wizard
routing, and context switching untested. The spec, meanwhile, was a
capture artifact nobody had measured the build against — a UX contract
with no reconciliation is a wish list.

## In Scope

- caret editing in the console input (`←/→`, Home/End, Delete)
- commands crossing the Local/Remote boundary automatically (spec §3)
- `converge profile [--set]`, profile in `status`, hints surfaced in the
  TUI
- reducer tests for caret, history recall, context crossing, wizard
  open/cancel/execute
- spec §8: built / intentionally different / deferred-with-triggers

## Out Of Scope

- the deferred items themselves, each recorded in spec §8 with the
  trigger that would justify building it

## Outcome

- **workflow profiles: built, narrowly.** They were a config field
  nothing could set or read — dead weight pretending to be a feature.
  Now `converge profile --set` writes it, `status` reports it, and the
  remote dashboard and Help show the profile's flow and release hints.
  Renaming domain nouns across every surface is deferred with a trigger:
  it multiplies every string by three profiles and would have to cover
  the CLI too, or the front-ends would speak different languages
- the caret fix found a real bug while being tested: the publish wizard
  emitted `--lane ""` for a blank field. Marking the field optional in
  17.3 was not enough — `build_argv` still pushed the flag. `--lane ""`
  is a lane id nobody owns, so it is now omitted, which is what makes
  the server resolve the personal lane
- boundary crossing is two lines in `submit`, and it removes the "wrong
  mode for this command" failure the spec calls out by name
- spec §8 splits three ways deliberately. "Intentionally different"
  carries the reasoning (Settings became Help because configuration is
  edited by verbs, and a second editing surface is a place for the two
  to disagree); "deferred" carries triggers, so a later reader can tell
  a decision from an omission
- 183 tests green, 38 of them over the reducer

## Next Task

Roadmap `g02.017` is complete. Open `g02.018` (adversarial test
hardening) — the last roadmap in the audit program.
