# 030 Interop Architecture

Status: ready
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

## Next Task

On completion, open the Batch 9.2 export card.
