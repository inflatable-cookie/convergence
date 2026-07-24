# 032 Git Import

Status: ready
Updated: 2026-07-24
Roadmap: `g02.009`
Spec: `docs/specs/009-git-interop.md`

## Objective

Doc 18 §3: seed a workspace from an existing git repo, with optional
first-parent history and ignore translation.

## In Scope

- `.convergeignore` support in capture (the one capture change): simple
  root-level patterns (name and `dir/` matches; no negations —
  documented), honored alongside built-ins
- `converge git import [--depth N | --all]`: default seeds the current
  git tree as the initial snap (`imported from git <sha[..12]>` +
  `Converge-Imported-Commit` trailer); depth walks first-parent
  oldest-first via `git rev-list`/`git show`, one snap per commit with
  wired lineage and preserved messages + trailer; populates the git map
  so a later export reuses history instead of duplicating it
- `.gitignore` (root) -> `.convergeignore` generation on import (skip
  negations, keep simple patterns; documented limitation)
- tests (git-on-PATH, graceful skip): seed import (tree matches, message
  trailer), --all import of 3 commits (lineage wired, messages kept),
  ignore translation honored by subsequent snap, import->export
  round-trip does not duplicate commits

## Out Of Scope

- coexistence polish (9.4)

## Acceptance Criteria

- a real git repo imports and works under Convergence; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- mapping gap — doc 18 first

## Next Task

On completion, open the Batch 9.4 coexistence card.
