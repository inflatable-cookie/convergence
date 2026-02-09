# Phase 081: God-File Decomposition (Wave 47)

## Goal

Decompose superpositions apply/validate and gate-graph actions into focused helper modules.

## Scope

Primary Wave 47 targets:
- `src/tui_shell/app/cmd_mode_actions/superpositions/apply_validate.rs` (~160 LOC)
- `src/tui_shell/app/cmd_gate_graph/actions.rs` (~159 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/cmd_mode_actions/superpositions/apply_validate.rs` into focused validate/apply helpers.
- [x] Split `src/tui_shell/app/cmd_gate_graph/actions.rs` into focused selection/edit/toggle helpers.
- [x] Preserve superposition resolution and gate-graph action behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo nextest run -E 'kind(lib)'` completed successfully (`15 passed, 0 failed`).
