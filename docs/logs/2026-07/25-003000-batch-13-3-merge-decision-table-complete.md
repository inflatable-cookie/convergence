# 2026-07-25 Batch 13.3 Complete — Merge Decision-Table Fix

Audit H4 (modify-back-to-W vs delete drops the modifier's opinion
instead of superposing with a Tombstone) is closed; card 048.

## What landed

- doc 17 §2 gained the rule first: restating W is still an opinion
  against a *concurrent* deletion. A `Set(k)` with `k == W` collapses
  into W only when no input deletes the path; against a concurrent
  `Delete` it superposes as the W-valued variant plus the Tombstone.
  The supersession exception is untouched — a deleter whose declared
  base already contains exactly `k` is causally newer and wins cleanly
- `merge_window`: the "sets that merely restate W" filter now applies
  only when no deleter contests the path. Content someone just
  affirmed can no longer be silently deleted by a concurrent delete
- new `decision_table` test file: nine named tests, one per doc 17 §2
  table cell — W passthrough, single modify, identical-set dedup,
  divergent superposition with lane provenance, lone delete,
  delete-vs-modify with Tombstone, unchanged-expresses-no-opinion, the
  fixed modify-back-to-W-vs-concurrent-delete cell, and W-restating
  collapse without deleters

## Validation

- `effigy validate` green: 128 tests; `effigy qa:docs` green

## Next Task

Open batch card 13.4 (resolution and identity hygiene): recursive
`validate_resolution`, field-match release deletion, recapture rule
fixes, length-prefixed parent encoding in `compute_snap_id`,
locked/CAS state.json updates.
