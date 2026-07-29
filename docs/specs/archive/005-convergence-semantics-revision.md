# 005 Convergence Semantics Revision

Status: complete
Updated: 2026-07-24
Roadmap: `g02.005`

## Context

Operator-approved improvement program (2026-07-24 review) laid out as
roadmaps `g02.005`-`g02.010` plus backlog. This spec governs `g02.005`, the
contract-changing tranche: snap lineage, base-aware merge, candidate windows,
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

- Batch 5.1 (architecture and vision revision) complete; doc 17 promoted.
- Batch 5.2 (snap lineage) complete.
- Batch 5.3 (base-aware merge and windows) complete.
- All four batches complete; roadmap `g02.005` closed.

## Exit Condition

Roadmap `g02.005` exit criteria met; `g02.006` opens.

## Next Task

Superseded by `docs/specs/006-continuous-capture-and-workspace-ux.md`.
