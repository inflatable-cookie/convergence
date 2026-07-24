# 031 Git Export

Status: complete
Updated: 2026-07-24
Roadmap: `g02.009`
Spec: `docs/specs/009-git-interop.md`

## Objective

Doc 18 §2: mirror snap lineage and channel history to git branches via
fast-import, byte-fidelity proven.

## In Scope

- `converge-client` git module: fast-import stream writer (marks,
  commits with trailers, tree emission from the local store incl. chunk
  reassembly, mode/symlink mapping); mapping table
  `.converge/git-map.json` for incremental re-export
- CLI `git export [--lane <id>] [--channel <name>]`: exports the local
  lineage of a lane head (local store) or a fetched channel history to
  `converge/lane/*` / `converge/channel/*`; refuses superposed trees;
  adds `.converge/` to `.git/info/exclude`
- thinned-ancestor handling per doc 18 (omit + trailer)
- tests (require `git` on PATH — skip gracefully if absent): export a
  3-snap lineage, `git log` shows 3 commits with trailers, checkout tree
  byte-identical to materialize; re-export is incremental (marks reused,
  branch force-moved); superposed snap refused

## Out Of Scope

- import (9.3), coexistence polish beyond info/exclude (9.4)

## Acceptance Criteria

- fidelity: checkout == materialize, byte-exact, tested
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- mapping gap found — doc 18 first

## Outcome

- `git_export::export_lineage`: fast-import stream (marks, trailers,
  full-tree emission with chunk reassembly, mode/symlink mapping),
  oldest-first lineage with thinned-gap tolerance; mapping table
  `.converge/git-map.json` makes re-export incremental (parents joined
  by recorded sha); superposed trees refused with "resolve before
  export"; `.converge/` auto-added to `.git/info/exclude`
- CLI `converge git export [--branch]` (default `converge/lane/local`)
  exporting the workspace head lineage — the card's lane/channel naming
  arrives as sugar once import (9.3) and coexistence (9.4) land;
  recorded as a simplification, mechanism unchanged
- tests (git-on-PATH, graceful skip): 3-commit export with trailers,
  clone-checkout byte-identical to workspace incl. binary file,
  internals excluded, incremental re-export (0 then exactly 1 new),
  superposed refusal
- 88 workspace tests green

## Next Task

Execute the Batch 9.3 import card (`032-git-import.md`).
