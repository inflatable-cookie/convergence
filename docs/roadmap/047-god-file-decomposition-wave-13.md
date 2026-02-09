# Phase 047: God-File Decomposition (Wave 13)

## Goal

Continue reducing remaining high-LOC wizard and command definition modules while preserving CLI/TUI behavior.

## Scope

Primary Wave 13 targets:
- `src/tui_shell/wizard/login_bootstrap_transitions.rs` (~278 LOC)
- `src/tui_shell/commands/mode_defs.rs` (~270 LOC)
- `src/model.rs` (~274 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Start with wizard transition flow split (`login_bootstrap_transitions.rs`).
- Follow with command defs split (`mode_defs.rs`).
- Defer `model.rs` until after low-risk slices land.

### B) Wizard Transition Decomposition
- [x] Split `src/tui_shell/wizard/login_bootstrap_transitions.rs` into login and bootstrap transition concerns.
- [x] Preserve wizard prompt progression and validation behavior.

Progress notes:
- Replaced `src/tui_shell/wizard/login_bootstrap_transitions.rs` with module directory:
  - `src/tui_shell/wizard/login_bootstrap_transitions/mod.rs`
  - `src/tui_shell/wizard/login_bootstrap_transitions/login.rs`
  - `src/tui_shell/wizard/login_bootstrap_transitions/bootstrap.rs`
- Preserved login/bootstrap text-input transition flow, validation, and final effect handoff.

### C) Command Definitions Decomposition
- [x] Split `src/tui_shell/commands/mode_defs.rs` by mode family.
- [x] Preserve command names, aliases, usage strings, and help text.

Progress notes:
- Replaced `src/tui_shell/commands/mode_defs.rs` with module directory:
  - `src/tui_shell/commands/mode_defs/mod.rs`
  - `src/tui_shell/commands/mode_defs/snaps_inbox.rs`
  - `src/tui_shell/commands/mode_defs/bundles_remote.rs`
  - `src/tui_shell/commands/mode_defs/superpositions_gate.rs`
- Preserved exported command-def function names and all command metadata text.

### D) Model Decomposition
- [ ] Split `src/model.rs` by config/snap/manifest/resolution concerns.
- [ ] Preserve serde shapes and public type API.

### E) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Progress notes:
- Validation for wizard + mode_defs slices:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo test --lib` passed (`15 passed`, `0 failed`)
