# 2026-07-25 02:30:00 BST - Batch 10.2 Batched Transport Complete

Roadmap: `g02.010`

## Summary

Object transfer is batched: CBOR frames with cap-splitting on upload and
wave-walk batch downloads. Per-object round trips are gone from the
client; hash verification discipline unchanged.

## Validation

- `effigy validate` — 94 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.010` Batch 10.3 event-push card.
