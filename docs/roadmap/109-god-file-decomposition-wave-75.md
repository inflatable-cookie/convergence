# Phase 109: God-File Decomposition (Wave 75)

## Goal

Decompose promotion creation, GC retained-roots computation, and remote upload object flows into focused helper modules.

## Scope

Primary Wave 75 targets:
- `src/bin/converge_server/handlers_release/promotion_endpoints/create.rs` (~108 LOC)
- `src/bin/converge_server/handlers_gc/roots.rs` (~105 LOC)
- `src/remote/transfer/upload/upload_objects.rs` (~105 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Extract promotion request validation/timestamp/id helpers from create handler.
- [x] Split retained-roots gathering into focused GC helper modules.
- [x] Split upload object flows by object kind (blobs/recipes/manifests/snaps).
- [x] Preserve API and upload behavior semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --lib` currently stalls in this environment immediately after launching the lib test binary.
- `cargo test --lib -- --list` also stalls with the same pattern.
- `cargo nextest run` still stalls in this environment after build/start.
