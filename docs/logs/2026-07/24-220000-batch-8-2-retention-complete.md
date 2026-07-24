# 2026-07-24 22:00:00 BST - Batch 8.2 Retention Policy Complete

Roadmap: `g02.008`

## Summary

Retention is control-plane config with pure, tested evaluation; client
thinning honors workspace retention settings. Nothing deletes yet — GC
(8.3) consumes these decisions.

## Validation

- `effigy validate` — 83 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.008` Batch 8.3 GC card.
