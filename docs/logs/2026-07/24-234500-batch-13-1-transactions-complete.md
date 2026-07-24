# 2026-07-24 Batch 13.1 Complete — Transactions

Audit H2 (no transaction spans publish/promote; the global mutex
serializes statements, not operations; Postgres auto-commits every
call) is closed; card 046, roadmap `g02.013` opened.

## What landed

- `MetaOp` write-batch enum and `apply_batch` on `MetadataStore`:
  AddPublication, PutBundle, SetPartitionState, RecordPromotion,
  AddEvent, plus guard ops AssertPartitionState and
  AssertPublicationCount. All ops commit in one transaction; a failed
  guard rolls everything back with a typed `BatchConflict`
- SQLite wraps the batch in `BEGIN IMMEDIATE`; Postgres in an explicit
  transaction. Statement helpers are shared between single-op methods
  and the batch path — one SQL source of truth per backend
- publish: read partition + window → compute the publication, its seq,
  and the merged bundle in memory → commit one guarded batch. A
  concurrent publish trips a guard, the batch rolls back, publish
  re-reads and rebuilds (bounded 32 attempts). `build_bundle` is now
  pure compute over a caller-supplied window
- promote: one guarded batch {assert partition as read, record
  promotion, advance window}; concurrent movement returns a clear
  conflict error instead of silent last-writer-wins
- publication seq made floor-aware in both backends (GREATEST of max
  seq and window floor + 1), so GC deleting consumed publications can
  never rewind seq assignment below the floor

## Validation

- `effigy validate` green: fmt, clippy, 117 tests passed (Postgres
  backend compiled under `backend-postgres`)
- new coverage: backend conformance for batched commit and
  guard-rollback semantics (a write executed before a failing guard is
  rolled back); HTTP e2e with 8 racing publishers to one partition —
  window ends come out exactly 1..=8, every window starts at the
  unmoved floor, and the final bundle folds all eight publications

## Next Task

Open batch card 13.2 (promotion guards): monotonic window floor,
base-must-match-W check; amend doc 14 §3 to state the actual
serialization mechanism.
