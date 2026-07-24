# 015 Semantics Architecture Revision

Status: ready
Updated: 2026-07-24
Roadmap: `g02.005`
Spec: `docs/specs/005-convergence-semantics-revision.md`

## Objective

Write the revised semantics into the architecture and vision before any
code: lineage, base-aware merge, bundle windows, gate strategies, and the
operator-agreed positioning.

## In Scope

- new `docs/architecture/17-lineage-and-merge-semantics.md`:
  - snap DAG: parents in the record, identity = hash(root manifest +
    parents), timestamp as metadata; head tracking rules
  - base-aware 3-way merge: `base_bundle_id` on publications, decision
    table (unchanged/one-sided/divergent/deleted), tombstone production
    and materialization semantics
  - bundle windows: window = publications since last promoted bundle;
    provenance records the window; interaction with determinism
  - per-gate coalesce strategy contract: strategy named on `GateNode`,
    dispatch rules per entry kind, `text-line-merge` and `whole-file`
    defined; strategy recorded in provenance
- revise doc 14 (partition build loop, promotion resetting the window) and
  doc 16 (wire DTO deltas) to match
- vision update (`docs/vision/001`): beachhead = binary-heavy small teams
  (DAW/game) with large-org gates as growth; git interop first-class;
  deterministic provenance replay as a named feature
- update `docs/architecture/README.md` index

## Out Of Scope

- any code (Batches 5.2-5.4)

## Acceptance Criteria

- doc 17 is decision-complete: an implementer needs no further semantic
  choices for Batches 5.2-5.4
- docs 14/16 contain no statements contradicted by doc 17
- `effigy qa:docs` / `qa:northstar` green

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Stop Conditions

- a semantics question needs operator intent (e.g. lineage identity edge
  cases with multiple parents) — ask, do not guess

## Next Task

On completion, open the Batch 5.2 snap-lineage card.
