# 017 Base-Aware Merge and Windows

Status: complete
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

## Outcome

- wire/model: `base_bundle_id` on publish/publication; `base_bundle_id` +
  `window` + `strategy` on bundles; `strategy` on `GateNode`; client
  tracks last-seen bundle per target and sends it automatically (updated
  on publish response and matching fetch)
- server: `partitions` state (window floor + W); base validated against
  the partition; builds fold the window onto W; promotion advances the
  floor and installs the promoted bundle as W
- merge rewritten as the doc 17 fold: per-input deltas vs declared base,
  no-opinion rule, clean deletions, delete-vs-modify tombstone
  superpositions
- one doc 17 revision via stop-condition, twice-refined: **supersession by
  base containment** — a Set is dropped when a causally-newer input built
  on that exact value AND the drop cannot lose content (the newer input
  has its own explicit opinion, or W carries the value). Kills
  false superpositions on sequential edits inside one window.
- 5 decision-table tests (sequential supersession, untouched publisher,
  clean deletion, delete-vs-modify tombstone, window reset + W install);
  53 workspace tests green incl. full e2e under new semantics

## Next Task

Execute the Batch 5.4 gate-strategies card (`018-gate-strategies.md`).
