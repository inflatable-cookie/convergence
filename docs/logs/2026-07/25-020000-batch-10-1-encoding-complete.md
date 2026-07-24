# 2026-07-25 02:00:00 BST - Batch 10.1 Canonical Encoding Complete

Roadmap: `g02.010`

## Summary

Hashed objects (manifests, recipes) now store as canonical CBOR with
magic prefixes; ids derive from canonical bytes everywhere (client,
server, git export). ~40% smaller than JSON on a 10k-entry tree. Paging
deferred to backlog with rationale; snap records honestly stay JSON.

## Validation

- `effigy validate` — 93 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.010` Batch 10.2 batched-transport card.
