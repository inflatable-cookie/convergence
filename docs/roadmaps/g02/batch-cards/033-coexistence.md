# 033 Coexistence

Status: ready
Updated: 2026-07-24
Roadmap: `g02.009`
Spec: `docs/specs/009-git-interop.md`

## Objective

Doc 18 §4 made real end to end: one tree, both systems, no fights — plus
the roadmap-closing e2e.

## In Scope

- roadmap exit e2e: real git repo -> `converge git import --all` ->
  work under Convergence (snap, publish to a server, resolve nothing) ->
  `converge git export` -> the mirror branch consumable by plain git
  (log, diff, checkout) with new Convergence-side commits present and no
  duplicated imports
- `converge status` gains a `git` block when `.git` is present: enclosing
  branch, whether head snap is mirrored (map lookup), mirror branch name
- guard: `converge git export` refuses when the workspace root is not
  the git root (nested confusion); clear error
- docs: coexistence quickstart in doc 18 (append §6 walkthrough)

## Out Of Scope

- anything bidirectional

## Acceptance Criteria

- exit e2e green; status shows git context; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- none specific

## Next Task

On completion, close roadmap `g02.009` against its exit criteria.
