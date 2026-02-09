# Phase 100: God-File Decomposition (Wave 66)

## Goal

Decompose mode hints, root command definitions, and CLI command enum wiring into focused helper modules.

## Scope

Primary Wave 66 targets:
- `src/tui_shell/app/default_actions/hints/mode_hints.rs` (~127 LOC)
- `src/tui_shell/commands/root_defs/global_local.rs` (~121 LOC)
- `src/cli_commands/mod.rs` (~123 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/default_actions/hints/mode_hints.rs` by mode-focused helpers.
- [x] Split `src/tui_shell/commands/root_defs/global_local.rs` by global/auth/local command groups.
- [x] Split `src/cli_commands/mod.rs` so command enum wiring lives in a focused submodule.
- [x] Preserve hint ordering and command definition semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built tests but stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
