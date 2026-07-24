# 2026-07-24 11:30:00 BST - Batch 5.2 Snap Lineage Complete

Roadmap: `g02.005`

## Summary

Snaps form a DAG: lineage-derived identity, head-tracked parents,
idempotent recapture, lineage-ordered history. One doc 17 amendment via
the stop-condition path: identical-tree recapture returns the head record
(the "same id" phrasing was mechanically impossible since the head is its
own parent's child).

## Validation

- `effigy validate` — 48 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.005` Batch 5.3 base-aware-merge-and-windows card.
