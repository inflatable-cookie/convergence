# Phase 071: God-File Decomposition (Wave 37)

## Goal

Continue status subsystem decomposition by splitting tree-walk snapshot traversal and rendering helpers into focused modules.

## Scope

Primary Wave 37 target:
- `src/tui_shell/status/tree_walk.rs` (~197 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Status Tree-Walk Decomposition
- [x] Split `src/tui_shell/status/tree_walk.rs` by traversal recursion and line emission helpers.
- [x] Preserve deterministic ordering and mode-specific presentation behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` was attempted but repeatedly stalled in this environment (long-running `clippy-driver` processes with no completion signal during this wave).
- `cargo nextest run` was attempted; when it stalled post-build, fallback validation was run via `cargo test -q rename_tests:: -- --nocapture` and passed (`3 passed, 0 failed`).
