# 2026-07-25 03:00:00 BST - Batch 10.3 Event Push Complete

Roadmap: `g02.010`

## Summary

The server tells clients what changed: durable per-repo event feed with
seq cursors, emitted on bundle/lane/release flows, polled by CLI and by
the TUI's background worker — blind remote refresh is gone. Events are
hints; the inbox stays the truth.

## Validation

- `effigy validate` — 95 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the Batch 10.4 external-backends card — the final batch of the
improvement program.
