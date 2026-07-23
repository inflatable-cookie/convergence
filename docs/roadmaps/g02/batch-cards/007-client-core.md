# 007 Client Core

Status: complete
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

## Outcome

- store: uniform sharded object fanout (`objects/<kind>/ab/cd/<hash>`), CRUD
  collapsed to one generic put/get/has layer with verify-on-read; flat g01
  layout not read (no migration by design)
- snap capture chunks large files with FastCDC via `converge_model::chunk_data`;
  fixed-block reader gone; `ChunkingConfig.chunk_size` maps to CDC average
  (min = avg/4, max = 4x avg)
- dead salvage cut: `workspace/gc`, `store/traversal` (archive keeps them);
  `#![allow(dead_code)]` lifted, zero allows in the workspace
- new tests: v2 CDC recipe + params on snap, sharded blob path, exact
  restore round-trip, cross-file dedup; g01 layout-dependent tests updated
- `effigy validate` green: fmt, clippy -D warnings, 14 nextest tests

## Next Task

Execute the Batch 3.3 CLI-verb-surface card (`008-cli-verb-surface.md`).
