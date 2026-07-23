# 007 Client Core

Status: ready
Updated: 2026-07-23
Roadmap: `g02.003`
Spec: `docs/specs/003-rebuild-vertical-slice.md`

## Objective

Finish the client core: sharded object storage, snap capture on FastCDC
recipes, and retire the salvage dead-code posture.

## In Scope

- sharded object fanout in the local store (`objects/ab/cd/<hash>`), with
  transparent read of the flat g01 layout removed (no migration — archive is
  history)
- snap capture path uses `converge_model::chunk_data` (FastCDC); remove the
  fixed-block chunking remnants (`chunking.rs` policy, fixed-size chunk_io
  paths)
- lift `#![allow(dead_code)]`: delete salvage code with no rebuild caller
  (e.g. g01-only GC entry points stay only if Batch 3.3+ verbs will call
  them; otherwise cut — archive keeps them)
- extend client tests: snap → restore round-trip over CDC recipes, sharded
  store read/write/dedup

## Out Of Scope

- CLI verbs (Batch 3.3), server (3.4), sync (3.5)

## Acceptance Criteria

- no `allow(dead_code)` in `converge-client`
- snap of a workspace with a large file produces a v2 CDC recipe; restore
  reproduces bytes exactly
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- salvage module needs behavior change (not deletion/rewiring) — route
  through architecture first

## Next Task

On completion, open the Batch 3.3 CLI-verb-surface card.
