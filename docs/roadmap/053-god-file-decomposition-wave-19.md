# Phase 053: God-File Decomposition (Wave 19)

## Goal

Continue decomposing dense view/server merge logic into focused modules while preserving coalescing, promotability, and UI detail behavior.

## Scope

Primary Wave 19 targets:
- `src/tui_shell/views/superpositions.rs` (~228 LOC)
- `src/bin/converge_server/object_graph/merge.rs` (~203 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

### B) Superpositions View Decomposition
- [x] Split `src/tui_shell/views/superpositions.rs` by row rendering and details rendering concerns.
- [x] Preserve selected-item details, decision markers, and validation summary rendering.

### C) Object Graph Merge Decomposition
- [x] Split `src/bin/converge_server/object_graph/merge.rs` by promotability and manifest-merging concerns.
- [x] Preserve coalescing behavior and promotability reason semantics.

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Verification notes:
- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
