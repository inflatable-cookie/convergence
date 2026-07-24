# 005 Convergence Semantics Revision

Status: active
Updated: 2026-07-24
Roadmap: `g02.005`

## Context

Operator-approved improvement program (2026-07-24 review) laid out as
roadmaps `g02.005`-`g02.010` plus backlog. This spec governs `g02.005`, the
contract-changing tranche: snap lineage, base-aware merge, bundle windows,
per-gate coalesce strategies — architecture and vision revised before code.

## Governing Refs

- `docs/roadmaps/g02/005-convergence-semantics-revision.md`
- `docs/architecture/13-16` (14 and 16 get revised by Batch 5.1)
- `docs/rebuild/001-lessons-retrospective.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

- docs first: no Batch 5.2+ code until the semantics doc is promoted
- pre-1.0: no compat shims, stores re-init; the archive is history
- determinism is a contract — every semantic change extends the
  determinism test set, never weakens it

## Sequencing Note

`g02.006`-`g02.010` are planned, not active. One active roadmap at a time;
each opens with its own spec and ready card at the previous close.

## Current State

- Batch 5.1 has a ready card:
  `docs/roadmaps/g02/batch-cards/015-semantics-architecture-revision.md`

## Exit Condition

Roadmap `g02.005` exit criteria met; `g02.006` opens.

## Next Task

Execute the ready Batch 5.1 card.
