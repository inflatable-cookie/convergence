# 030 Interop Architecture

Status: complete
Updated: 2026-07-24
Roadmap: `g02.009`
Spec: `docs/specs/009-git-interop.md`

## Objective

Decision-complete mapping contract between Convergence and git, promoted
as `docs/architecture/18-git-interop.md` before any code.

## In Scope

- mapping: snap lineage <-> commits (identity correspondence, timestamp
  handling given metadata-only created_at), bundles <-> merge commits,
  tombstones <-> deletions, superpositions (unrepresentable in git —
  define the export rule: refuse, or export resolved-only trees)
- boundary rules: what Convergence owns vs mirrors; `.git` and
  `.converge` in one tree (capture ignores git internals — already true;
  git ignores `.converge` via export-managed info/exclude)
- export contract: fast-import stream shape, branch naming
  (`converge/<lane|channel>`), round-trip fidelity definition
- import contract: seeding depth, ignore translation, author mapping
- staged non-goals: bidirectional live sync, submodules, LFS

## Out Of Scope

- implementation (9.2-9.4)

## Acceptance Criteria

- doc 18 decision-complete for batches 9.2-9.4; docs QA green

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Stop Conditions

- mapping requires doc 17 changes — revise 17 first

## Outcome

- `docs/architecture/18-git-interop.md` promoted, decision-complete:
  trailer-based identity correspondence with a local mapping table;
  snap->commit and bundle->merge-commit export with thinned-ancestor
  visibility; **superposed trees refuse to export** (no lossy
  flattening); read-only force-moved mirror branches
  (`converge/lane/*`, `converge/channel/*`); byte-fidelity contract;
  first-parent history import with `.convergeignore` generation; one
  capture change scoped (ignore-file support); coexistence exclusion
  rules; fast-import/plumbing tooling shape, client-side only
- staged non-goals recorded: bidirectional sync, submodules, LFS
  translation

## Next Task

Execute the Batch 9.2 export card (`031-git-export.md`).
