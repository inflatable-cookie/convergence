# 016 Snap Lineage

Status: complete
Updated: 2026-07-24
Roadmap: `g02.005`
Spec: `docs/specs/005-convergence-semantics-revision.md`

## Objective

Implement doc 17 §1: the snap DAG with lineage-derived identity.

## In Scope

- `SnapRecord` v2: `parents`, `derived_from_bundle`, identity =
  `blake3("converge-snap-v2\n" + root_manifest + parents + derived_from)`;
  `created_at`/`message`/`trigger` metadata only
- workspace: capture sets `parents = [head]` and advances head; unchanged
  tree over the same head returns the existing snap (no duplicate);
  restore moves head
- history: lineage-ordered rendering (parent walk, `created_at` for
  parallel-branch display only)
- tests: identity stability (same tree+parents -> same id; different
  parent -> different id), idempotent recapture, head movement on
  capture/restore, message edit not changing identity

## Out Of Scope

- publication base / windows (5.3), strategies (5.4)

## Acceptance Criteria

- doc 17 §1 consequences all hold in tests
- full suite green (`effigy validate`)

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- semantics gap found in doc 17 — revise the doc first

## Outcome

- `SnapRecord` v2: `parents` + `derived_from_bundle`;
  `compute_snap_id(root, parents, derived)` with domain-separated hash;
  timestamp/message metadata only
- capture: parents from head; identical-tree-vs-head recapture returns the
  head record (doc 17 amended per stop-condition — the original "same id"
  phrasing was mechanically wrong, outcome unchanged)
- restore moves head; post-restore capture branches from the restored snap
- `list_snaps` is lineage-ordered: head-first parent walk, parallel
  branches appended newest-first; CLI history consumes it
- 5 lineage tests (identity, idempotent recapture, head movement,
  lineage-vs-timestamp ordering, message-edit identity stability);
  48 workspace tests green

## Next Task

Execute the Batch 5.3 card (`017-base-aware-merge-and-windows.md`).
