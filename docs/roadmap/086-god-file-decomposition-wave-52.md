# Phase 086: God-File Decomposition (Wave 52)

## Goal

Decompose remote listing openers, superposition decision actions, and inbox view rendering into focused modules.

## Scope

Primary Wave 52 targets:
- `src/tui_shell/app/remote_list_views.rs` (~145 LOC)
- `src/tui_shell/app/superpositions_nav/decisions.rs` (~143 LOC)
- `src/tui_shell/views/inbox.rs` (~144 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/remote_list_views.rs` into inbox/bundles helpers.
- [x] Split `src/tui_shell/app/superpositions_nav/decisions.rs` into focused clear/pick helpers.
- [x] Split `src/tui_shell/views/inbox.rs` into focused render/list/detail helpers.
- [x] Preserve existing TUI flow and action behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo test --lib` completed successfully (`15 passed, 0 failed`).
