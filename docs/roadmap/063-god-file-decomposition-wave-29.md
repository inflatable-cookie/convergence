# Phase 063: God-File Decomposition (Wave 29)

## Goal

Continue decomposing workspace orchestration by splitting root workspace lifecycle/snap/materialization methods into focused modules.

## Scope

Primary Wave 29 target:
- `src/workspace.rs` (~198 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Workspace Root Decomposition
- [x] Split `src/workspace.rs` by init/discovery, snap lifecycle, restore/materialize, and current-manifest query concerns.
- [x] Preserve restore safety checks, HEAD behavior, and materialization force semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- 2026-02-09: `cargo fmt` passed.
- 2026-02-09: `cargo clippy --all-targets -- -D warnings` passed.
- 2026-02-09: `cargo nextest run` compiled and then stalled in this environment after build completion; fallback `cargo test --lib` passed (15 passed, 0 failed).
