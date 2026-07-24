# 2026-07-24 13:00:00 BST - Batch 5.3 Base-Aware Merge and Windows Complete

Roadmap: `g02.005`

## Summary

Merge has a base, deletions are real, and bundle input sets are bounded by
promotion windows. Two doc 17 refinements landed through the stop-condition
path, both around supersession by base containment (drop rule initially
missing; first version could lose content carried only by a superseded
opinion).

## Validation

- `effigy validate` — 53 nextest tests green (incl. full e2e)
- `effigy qa:docs` — green

## Next Task

Execute the `g02.005` Batch 5.4 gate-strategies card.
