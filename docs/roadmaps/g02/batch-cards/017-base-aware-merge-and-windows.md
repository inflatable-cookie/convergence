# 017 Base-Aware Merge and Windows

Status: ready
Updated: 2026-07-24
Roadmap: `g02.005`
Spec: `docs/specs/005-convergence-semantics-revision.md`

## Objective

Implement doc 17 §2-3: publications declare their base, bundle builds are
3-way folds onto W over a promotion-bounded window, deletions become real.

## In Scope

- wire/model: `base_bundle_id` on `PublishRequest`/`PublicationRecord`;
  `base_bundle_id` + `window` + `strategy` on `BundleRecord`
- client: track last-seen bundle per `(repo, scope, gate)` target (state);
  send it as base automatically; update on publish response and fetch
- server: validate base against partition history; partition
  `window_floor` state; build consumes publications with
  `seq > window_floor`; promotion advances the floor and makes the
  promoted bundle the new W
- merge: per-input delta vs declared base (Merkle short-circuit), fold per
  doc 17 §2 decision table — unchanged expresses no opinion, one-sided
  wins, divergence superposes, clean deletion removes the path, delete-vs-
  modify superposes with a `Tombstone` variant
- resolution: resolving to a tombstone variant removes the path
- tests: deletion propagation e2e, delete-vs-modify tombstone
  superposition, no-opinion rule (untouched publisher never collides),
  window reset on promotion, determinism with base+window in the id

## Out Of Scope

- coalesce strategies beyond `whole-file` (5.4)

## Acceptance Criteria

- doc 17 §2 decision table fully covered by tests
- e2e green under new semantics; determinism holds

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- semantics gap in doc 17 — revise the doc first

## Next Task

On completion, open the Batch 5.4 gate-strategies card.
