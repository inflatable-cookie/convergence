# 2026-07-25 Batch 13.2 Complete — Promotion Guards

Audit H1 (promote lacks monotonicity guards — a stale bundle rewinds
the window and re-opens consumed publications) is closed; card 047.

## What landed

- doc 14 §3 amended first: partition serialization is optimistic
  guarded batches (assert partition + window, roll back on conflict,
  bounded retry) over transactional connections — not the row-lock
  single writer the doc previously described. Promotion monotonicity
  added as a stated invariant
- promote guards, evaluated against the same partition read the batch
  asserts (so they cannot be raced): `bundle.window.1 > floor`, else
  "stale bundle … republish against the current W"; bundle base must
  equal the partition's current W, else "would fork promoted history"
- fan-out re-promotion stays legal: a bundle that already is the
  current W promotes to further downstream gates by recording the
  promotion only — partition state untouched

## Validation

- `effigy validate` green: 119 tests; `effigy qa:docs` green
- new coverage: stale promote refused after a newer bundle promoted
  (floor never rewinds), wrong-base promote refused, current-W
  re-promotes to a second downstream gate without state change

## Next Task

Open batch card 13.3 (merge decision-table fix): modify-vs-delete
superposes per doc 17 — doc first, then `merge_window`; regression
tests for every cell of the decision table.
