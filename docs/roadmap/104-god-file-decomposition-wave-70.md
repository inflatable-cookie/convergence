# Phase 104: God-File Decomposition (Wave 70)

## Goal

Decompose server object-graph traversal/merge internals and persistence default/backfill helpers into focused modules.

## Scope

Primary Wave 70 targets:
- `src/bin/converge_server/object_graph/traversal/collect.rs` (~127 LOC)
- `src/bin/converge_server/object_graph/merge/manifest_merge.rs` (~127 LOC)
- `src/bin/converge_server/persistence/defaults_backfill.rs` (~126 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/bin/converge_server/object_graph/traversal/collect.rs` into recipe/manifest traversal helpers.
- [x] Split `src/bin/converge_server/object_graph/merge/manifest_merge.rs` into grouped merge/variant helper steps.
- [x] Split `src/bin/converge_server/persistence/defaults_backfill.rs` into provenance/ACL/default-repo helpers.
- [x] Preserve traversal/merge/backfill behavior and validation semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built test binaries but stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
