# 2026-07-24 23:50:00 BST - Batch 8.4 Verify Complete; g02.008 Closed

Roadmap: `g02.008`

## Summary

The determinism feature is live: `converge verify <bundle>` replays the
recorded merge and proves the identity; tampered provenance fails loudly.
The releases/retention/GC roadmap closes — the six-verb contract runs end
to end with honest storage. `g02.009` (git interop) opens docs-first.

## Validation

- `effigy validate` — 86 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.009` Batch 9.1 interop-architecture card.
