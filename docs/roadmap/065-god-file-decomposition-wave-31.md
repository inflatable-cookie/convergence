# Phase 065: God-File Decomposition (Wave 31)

## Goal

Continue reducing dense resolution/object-graph modules by splitting resolution application internals and object graph store access/validation helpers into focused submodules.

## Scope

Primary Wave 31 targets:
- `src/resolve/apply.rs` (~182 LOC)
- `src/bin/converge_server/object_graph/store.rs` (~183 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

### B) Resolution Apply Decomposition
- [x] Split `src/resolve/apply.rs` by validation precheck, decision-index resolution, and manifest rewrite recursion.
- [x] Preserve error text and deterministic manifest output semantics.

### C) Object Graph Store Decomposition
- [x] Split `src/bin/converge_server/object_graph/store.rs` by manifest entry reference validation, object reads, and manifest writes.
- [x] Preserve object-id validation and integrity-check behavior.

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- 2026-02-09: `cargo fmt` passed.
- 2026-02-09: `cargo clippy --all-targets -- -D warnings` passed.
- 2026-02-09: `cargo nextest run` compiled and then stalled in this environment after build completion; fallback `cargo test --lib` passed (15 passed, 0 failed).
