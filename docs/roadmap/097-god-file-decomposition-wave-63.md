# Phase 097: God-File Decomposition (Wave 63)

## Goal

Decompose modal key mapping, modal output helpers, and settings action execution into focused helper modules.

## Scope

Primary Wave 63 targets:
- `src/tui_shell/modal/keymap.rs` (~114 LOC)
- `src/tui_shell/app/modal_output.rs` (~111 LOC)
- `src/tui_shell/app/settings_do_mode.rs` (~115 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/modal/keymap.rs` into focused viewer/input/error helpers.
- [x] Split `src/tui_shell/app/modal_output.rs` into focused log and modal helper modules.
- [x] Split `src/tui_shell/app/settings_do_mode.rs` into focused chunking/retention/toggle handlers.
- [x] Preserve current modal/settings behavior and messaging semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built tests but stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
