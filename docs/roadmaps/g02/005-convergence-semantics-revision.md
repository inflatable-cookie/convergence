# 005 Convergence Semantics Revision

Status: active
Owner: repo maintainers
Updated: 2026-07-24

## Context

Building the rebuild exposed four contract-level gaps the docs never showed
(operator review, 2026-07-24). All four change model/wire contracts — cheap
now, brutal after real data exists — so they land before any feature roadmap:

1. snaps have no lineage (no parent links; timestamp inside snap identity)
2. merge has no base (union semantics; `Tombstone` never produced; deletions
   indistinguishable from never-existed)
3. bundles rebuild from every publication ever (unbounded input growth)
4. one hardcoded merge strategy (gates should own their coalesce policy;
   whole-file superpositions on one-line text edits are worse than git)

The same pass updates the vision with the operator-agreed positioning:
binary-heavy small teams (DAW/game) as the adoption beachhead with large-org
gates as the growth story, git interop as first-class, and deterministic
provenance replay as the enterprise feature.

## Goals

- revise architecture (docs 14/16 + a new semantics doc) and the vision
  before touching code
- snap lineage: parent snap ids in the record; snap identity derived from
  content + lineage, timestamp as metadata
- base-aware 3-way merge: publications carry `base_bundle_id`; tombstones
  produced for true deletions; false superpositions eliminated
- bundle windows: inputs = publications since the last promoted bundle (or
  explicit gate collection policy)
- per-gate coalesce strategies: line-merge for text (superpose only true
  conflicts), whole-file for binary, strategy named in gate policy and
  recorded in bundle provenance

## Non-Goals

- lanes (g02.007), auto-capture (g02.006), releases/GC (g02.008)
- data migration — pre-1.0, no compat shims; stores re-init

## Execution Plan

### Batch 5.1 - Architecture and Vision Revision

- [ ] new `docs/architecture/17-lineage-and-merge-semantics.md`: snap DAG,
      identity derivation, base-aware 3-way merge, tombstone semantics,
      bundle windowing, per-gate strategy contract
- [ ] revise docs 14/16 where the above changes them (partition build loop,
      wire DTOs)
- [ ] vision update: beachhead positioning, git interop as first-class,
      determinism/provenance-replay as a named product feature

### Batch 5.2 - Snap Lineage

- [ ] `SnapRecord` gains `parents: Vec<String>`; id = hash(root manifest +
      parents); `created_at` metadata only
- [ ] workspace tracks current head as parent for the next snap; restore
      sets head; history renders lineage order

### Batch 5.3 - Base-Aware Merge and Windows

- [ ] `PublishRequest`/`PublicationRecord` gain `base_bundle_id`
- [ ] 3-way manifest merge (base vs each input): unchanged-vs-base drops
      out, single-sided change wins, true divergence superposes, deletion
      vs base produces `Tombstone`
- [ ] bundle windows: partition tracks last promoted bundle; builds consume
      publications after it; window recorded in provenance

### Batch 5.4 - Gate Coalesce Strategies

- [ ] strategy contract on `GateNode` (e.g. `text-line-merge`,
      `whole-file`); engine dispatches per entry
- [ ] line-merge for text blobs with true-conflict-only superpositions
- [ ] strategy + inputs recorded in bundle provenance; determinism tests
      extended per strategy

## Exit Criteria

- architecture/vision revised before implementation started
- e2e suite green under the new semantics, incl. deletion propagation and
  a text edit from two lanes producing a line-merged file (no superposition)
  vs a true conflict producing one
- deterministic bundle ids under windows + strategies

## Next Task

Execute the ready Batch 5.1 card
(`batch-cards/015-semantics-architecture-revision.md`).
