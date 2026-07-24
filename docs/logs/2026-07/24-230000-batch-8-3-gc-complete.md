# 2026-07-24 23:00:00 BST - Batch 8.3 GC Complete

Roadmap: `g02.008`

## Summary

Mark-and-sweep GC with dry-run-first discipline: retention drops metadata
for the triggering repo, the mark spans all repos (shared content-
addressed object store), and the sweep respects an mtime grace window.
The reachable-never-collected invariant is proven by test before any
production sweep exists.

## Validation

- `effigy validate` — 85 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.008` Batch 8.4 provenance-verify card.
