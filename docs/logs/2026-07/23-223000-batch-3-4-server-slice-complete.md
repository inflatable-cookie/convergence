# 2026-07-23 22:30:00 BST - Batch 3.4 Server Slice Complete

Roadmap: `g02.003`

## Summary

The convergence engine exists: pluggable embedded storage, authz by
construction, deterministic Merkle-merge bundle builds with
superposition-as-data, and policy-checked promotion. The operation the g01
server stubbed for its whole life — computing a coalesced bundle manifest —
now runs and is tested.

## Changes

- `converge-server`: `storage.rs` traits, `object_fs.rs` (sharded FS,
  verify-on-read), `meta_sqlite.rs` (SQLite; per-partition publication
  sequencing), `authz.rs` (`AuthzContext` only via `authorize`),
  `merge.rs` (deterministic merge; divergence -> superposition with
  per-lane provenance), `engine.rs` (publish -> build -> approve ->
  promote)
- `PublicationRecord` gains `root_manifest`
- 6 integration tests incl. determinism across fresh stores and both
  promotion-block paths
- opened ready card `010-end-to-end-sync.md`

## Validation

- `effigy validate` — fmt, clippy -D warnings, 23 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.003` Batch 3.5 end-to-end sync card.
