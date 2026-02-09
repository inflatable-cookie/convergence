# Phase 106: God-File Decomposition (Wave 72)

## Goal

Decompose bootstrap handling, publication endpoints, and identity-store helpers into focused modules.

## Scope

Primary Wave 72 targets:
- `src/bin/converge_server/handlers_system/bootstrap.rs` (~120 LOC)
- `src/bin/converge_server/handlers_publications/publications.rs` (~124 LOC)
- `src/bin/converge_server/identity_store.rs` (~111 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/bin/converge_server/handlers_system/bootstrap.rs` into auth/creation/persistence helpers.
- [x] Split `src/bin/converge_server/handlers_publications/publications.rs` into create/list and validation helpers.
- [x] Split `src/bin/converge_server/identity_store.rs` into timestamp/hash, disk IO, and bootstrap helpers.
- [x] Preserve bootstrap/publication/identity semantics and response shapes.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --lib` passed.
- `cargo nextest run` still stalls in this environment after build completes; fallback library tests are green.
