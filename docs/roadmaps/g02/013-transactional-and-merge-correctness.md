# 013 Transactional and Merge Correctness

Status: complete
Owner: repo maintainers
Updated: 2026-07-24

## Context

Neither metadata backend uses a single transaction anywhere; multi-step
operations (publish, promote, GC bookkeeping) interleave under
concurrency and can commit mutually inconsistent state. Promote has no
monotonicity guard, so a stale bundle rewinds the window. The merge
engine has one decision-table hole (modify-vs-delete collapses to a
clean delete), and resolution validation misses nested superpositions.
These corrupt promoted history silently — worse than crashing.

## Findings Addressed

- H1: promote lacks `window.1 > floor` and base==current-W guards —
  window regression re-opens consumed publications
- H2: no transaction spans publish/promote read-modify-write; the
  global mutex serializes statements, not operations; Postgres backend
  auto-commits each call
- H4: modify-back-to-W vs delete drops the modifier's opinion instead
  of superposing with a Tombstone (doc 17 breach)
- C1 (client): `validate_resolution` never descends into Dir variants —
  validate passes, apply fails on the nested superposition
- M1: `delete_releases_for_bundles` matches by JSON `LIKE '%id%'` —
  over-deletes releases, cascading into GC sweeping live objects
- C3 (client): idempotent recapture drops explicit messages on
  unchanged trees, misses dedup when HEAD unset, and `put_snap`
  overwrite discards second-writer metadata
- C2 (client): unlocked read-modify-write of `state.json` loses
  concurrent updates, staling the merge base pointer
- L3: snap-id parent join not length-prefixed (latent canonicalization
  weakness)

## Execution Plan (batch details in cards)

- **13.1 Transactions** (complete, card 046): `MetaOp`/`apply_batch`
  with guard ops in both backends (SQLite `BEGIN IMMEDIATE`, Postgres
  explicit transactions); publish and promote each one atomic guarded
  batch, publish retries on conflict, floor-aware publication seq
- **13.2 Promotion guards** (complete, card 047): monotonic window
  floor and base-must-match-W checks with fan-out re-promotion; doc 14
  §3 states the actual serialization mechanism (guarded batches)
- **13.3 Merge decision-table fix** (complete, card 048):
  modify-back-to-W vs concurrent delete superposes with a Tombstone
  per doc 17 (rule added there first); `decision_table` regression
  file covers every cell
- **13.4 Resolution and identity hygiene** (complete, card 049):
  decision-aware `validate_resolution`; field-match release deletion;
  recapture rules (explicit message lands on the head record, dedup
  without HEAD, write-once snap records); length-prefixed parent
  encoding in `compute_snap_id`; lock-guarded state.json updates

## Exit Criteria (all met)

- two concurrent publishes plus a mid-build promote to one partition
  produce consistent window/base state under test (13.1: eight racing
  publishers, distinct window ends, guarded batches)
- stale-bundle promote is refused with a clear error (13.2)
- full merge decision table covered by tests, including the
  modify-vs-delete cell and nested-superposition validate/apply parity
  (13.3 `decision_table`, 13.4 `hygiene`)

## Next Task

Roadmap complete. Open `g02.014` (architecture honesty).
