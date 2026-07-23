# 2026-07-23 20:45:00 BST - Batch 3.2 Client Core Complete

Roadmap: `g02.003`

## Summary

Client core finished: sharded content store, FastCDC snap capture, salvage
dead-code posture retired.

## Changes

- `LocalStore`: sharded fanout `objects/<kind>/ab/cd/<hash>`; per-kind CRUD
  files collapsed into one generic verified put/get/has layer; `.json`
  suffixes dropped (names are pure hashes); flat g01 layout not read
- snap capture: fixed-block chunker replaced by `converge_model::chunk_data`
  (FastCDC) in both store-backed and in-memory scan paths;
  `ChunkingConfig.chunk_size` maps to the CDC average
- cut dead salvage (`workspace/gc`, `store/traversal`); zero
  `allow(dead_code)` remains
- tests: CDC recipe v2 + header params, sharded path shape, byte-exact
  restore, cross-file dedup; two g01 tests updated off flat-layout
  assumptions
- opened ready card `008-cli-verb-surface.md`

## Validation

- `effigy validate` — fmt, clippy -D warnings, 14 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.003` Batch 3.3 CLI-verb-surface card.
