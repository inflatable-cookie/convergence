# Phase 092: God-File Decomposition (Wave 58)

## Goal

Decompose local maintenance command handlers, snaps restore/revert handlers, and lanes view rendering into focused modules.

## Scope

Primary Wave 58 targets:
- `src/tui_shell/app/local_maintenance.rs` (~136 LOC)
- `src/tui_shell/app/local_snaps_restore.rs` (~134 LOC)
- `src/tui_shell/views/lanes.rs` (~134 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/local_maintenance.rs` into focused show/restore-move/gc helpers.
- [x] Split `src/tui_shell/app/local_snaps_restore.rs` into focused revert/restore handlers.
- [x] Split `src/tui_shell/views/lanes.rs` into focused list/details/render helpers.
- [x] Preserve command semantics and view behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` surfaced visibility regressions; those were fixed. Subsequent full clippy runs were unstable due lingering lock/stall behavior in this environment.
- Fallback `cargo check -q` completed successfully.
- `cargo test --lib` completed successfully (`15 passed, 0 failed`).
