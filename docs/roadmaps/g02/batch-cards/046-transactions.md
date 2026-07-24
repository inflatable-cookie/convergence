# 046 Transactions

Status: complete
Updated: 2026-07-24
Roadmap: `g02.013`

## Objective

Audit H2 closed: publish and promote become single atomic metadata
operations in both backends. Today the global mutex serializes
statements, not operations — concurrent publishes to one partition can
interleave add-publication / build / put-bundle and commit mutually
inconsistent window state; Postgres auto-commits every call.

## In Scope

- `MetaOp` write-batch enum + `apply_batch(&[MetaOp])` on
  `MetadataStore`: AddPublication, PutBundle, SetPartitionState,
  RecordPromotion, AddEvent, plus guard ops AssertPartitionState and
  AssertPublicationCount that abort the batch when violated
- SQLite: one locked connection, `BEGIN IMMEDIATE` transaction around
  the batch; Postgres: explicit transaction (no more per-statement
  auto-commit); shared statement helpers so single-op methods and the
  batch path use the same SQL
- publish reworked: read window → compute publication seq, merged
  bundle, and window range in memory → one guarded batch
  {assert partition state + publication count, add publication, put
  bundle, add event}; guard failure re-reads and rebuilds (bounded
  retries) so a concurrent publish never commits a stale window
- promote reworked: one guarded batch {assert partition state as read,
  record promotion, set partition state}; conflict is a clear error,
  not silent last-writer-wins
- tests: backend conformance for apply_batch (atomicity: failing guard
  rolls back all writes; both backends via the shared suite); engine
  test with racing concurrent publishes to one partition — final
  window/base state consistent, every publication in exactly one
  bundle window chain

## Out Of Scope

- monotonic floor / base==W semantic promotion guards and doc 14 §3
  amendment (13.2); merge decision table (13.3); client-side state.json
  locking (13.4)

## Acceptance Criteria

- concurrent publishes to one partition produce consistent
  window/floor/base state under test; guard-violation batch leaves no
  partial writes; all suites green

## Validation

- `effigy validate`

## Outcome

- `MetaOp` + `apply_batch` on `MetadataStore`: writes commit together or
  not at all; guard ops (`AssertPartitionState`,
  `AssertPublicationCount`) roll the batch back with a typed
  `BatchConflict`. SQLite runs `BEGIN IMMEDIATE`; Postgres an explicit
  transaction (auto-commit-per-statement gone). Shared statement
  helpers keep one SQL source of truth per backend
- publish: read window → compute publication + merged bundle in memory
  → one guarded batch {asserts, add publication, put bundle, event};
  conflict re-reads and rebuilds (32 bounded attempts). Publication seq
  is floor-aware in both backends (GC below the floor can no longer
  rewind seq)
- promote: one guarded batch {assert partition unchanged, record
  promotion, advance window}; concurrent movement is a clear error
- `build_bundle` is pure compute over a caller-supplied window — no
  metadata writes outside the batch
- tests: conformance covers batched commit + guard rollback (write
  before a failed guard rolled back) on both backends via the shared
  suite; 8 racing publishers over HTTP serialize into distinct window
  ends 1..=8 with the final bundle folding all publications; 117 tests
  green

## Next Task

Batch card 13.2 (promotion guards).
