# Phase 099: God-File Decomposition (Wave 65)

## Goal

Decompose settings text-input actions, settings view core types/render wiring, and wizard type definitions into focused modules.

## Scope

Primary Wave 65 targets:
- `src/tui_shell/app/cmd_text_input/settings_actions.rs` (~118 LOC)
- `src/tui_shell/views/settings/mod.rs` (~117 LOC)
- `src/tui_shell/wizard/types.rs` (~111 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/cmd_text_input/settings_actions.rs` into chunking and retention helpers.
- [x] Split `src/tui_shell/views/settings/mod.rs` into view core and rendering helpers.
- [x] Split `src/tui_shell/wizard/types.rs` into grouped wizard type modules with re-exports.
- [x] Preserve settings/wizard behavior and public type surface.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built tests but stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
