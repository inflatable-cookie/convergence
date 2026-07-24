# 031 Git Export

Status: ready
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

## Next Task

On completion, open the Batch 9.3 import card.
