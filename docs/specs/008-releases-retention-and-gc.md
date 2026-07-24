# 008 Releases, Retention, and GC

Status: active
Updated: 2026-07-24
Roadmap: `g02.008`

## Governing Refs

- `docs/roadmaps/g02/008-releases-retention-and-gc.md`
- `docs/architecture/14-server-authority-and-distribution.md` (GC posture)
- `docs/architecture/17-lineage-and-merge-semantics.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

`release` completes the six-verb contract; retention and GC make storage
honest. GC is mark-then-sweep with a dry-run-first discipline — nothing
reachable may ever be collected, proven by tests before any sweep runs in
anger.

## Current State

- Batch 8.1 (release channels) complete — the six-verb contract is now
  fully implemented end to end.
- Batch 8.2 (retention policy) complete.
- Batch 8.3 has a ready card:
  `docs/roadmaps/g02/batch-cards/028-gc.md`

## Exit Condition

Roadmap `g02.008` exit criteria met.

## Next Task

Execute the ready Batch 8.3 card.
