# 048 Merge Decision-Table Fix

Status: complete
Updated: 2026-07-25
Roadmap: `g02.013`

## Objective

Audit H4 closed: `merge_window` drops W-restating `Set` opinions before
checking for deleters, so "modify back to W's value" vs "delete"
collapses into a clean delete — the modifier's explicit keep opinion is
silently discarded (doc 17 breach: modify-vs-delete must superpose with
a `Tombstone`).

## In Scope

- doc 17 §2 first: rules gain the explicit sentence that restating W is
  still an opinion against a deletion — a `Set(k)` with `k == W` only
  collapses into W when no input deletes the path
- `merge_window`: the "sets that merely restate W" filter applies only
  when there are no deleters; with a contested delete the W-valued
  variant survives into the superposition alongside the Tombstone
- regression test file covering every decision-table cell: W
  passthrough, single add/modify, identical-set dedup, divergent
  superposition, clean delete, delete-vs-modify with Tombstone,
  unchanged-no-opinion, and the fixed modify-back-to-W-vs-delete cell

## Out Of Scope

- resolution validation, recapture, snap-id encoding, state.json
  locking (13.4); strategy changes (text-line-merge untouched)

## Acceptance Criteria

- modify-back-to-W vs delete produces a superposition {W-value variant,
  Tombstone}, not a silent delete; every table cell has a named test;
  all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- doc 17 §2 rule added first: restating W is an opinion against a
  *concurrent* deletion — collapses into W only when no input deletes
  the path; a deleter whose base contains the exact value stays
  causally newer and wins via supersession (unchanged)
- `merge_window`: the W-restating-set filter now applies only when no
  deleter contests the path; against a concurrent delete the W-valued
  variant survives into the superposition with the Tombstone
- new `decision_table` test file — nine named tests, one per table
  cell: W passthrough, single modify, identical-set dedup, divergent
  superposition (lane provenance), lone delete, delete-vs-modify with
  Tombstone, unchanged-no-opinion, the fixed
  modify-back-to-W-vs-concurrent-delete cell, and W-restating collapse
  without deleters; 128 tests green

## Next Task

Batch card 13.4 (resolution and identity hygiene).
