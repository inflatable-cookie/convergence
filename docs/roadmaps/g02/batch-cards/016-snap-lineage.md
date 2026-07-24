# 016 Snap Lineage

Status: ready
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

## Next Task

On completion, open the Batch 5.3 base-aware-merge-and-windows card.
