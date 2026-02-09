# Phase 051: God-File Decomposition (Wave 17)

## Goal

Continue reducing high-LOC structural files by splitting server domain types and superposition navigation flows into focused modules with stable behavior.

## Scope

Primary Wave 17 targets:
- `src/bin/converge_server/types.rs` (~246 LOC)
- `src/tui_shell/app/superpositions_nav.rs` (~218 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Decompose server types by domain grouping first.
- Decompose superpositions navigation by mutation vs jump concerns second.

### B) Server Types Decomposition
- [x] Split `src/bin/converge_server/types.rs` into focused type-group modules.
- [x] Preserve all existing type visibility and serde behavior.

### C) Superpositions Navigation Decomposition
- [x] Split `src/tui_shell/app/superpositions_nav.rs` by decision mutation and navigation jump concerns.
- [x] Preserve selected-item behavior, resolution writes, and validation updates.

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Verification notes:
- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
