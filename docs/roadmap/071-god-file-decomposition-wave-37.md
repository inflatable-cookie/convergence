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
- [ ] Split `src/tui_shell/status/tree_walk.rs` by traversal recursion and line emission helpers.
- [ ] Preserve deterministic ordering and mode-specific presentation behavior.

### C) Verification and Hygiene
- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [ ] Keep this phase doc updated as slices land.
