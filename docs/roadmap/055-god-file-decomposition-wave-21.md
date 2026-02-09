# Phase 055: God-File Decomposition (Wave 21)

## Goal

Continue reducing dense remote command execution modules by splitting per-command mutation logic and transfer logic into focused submodules.

## Scope

Primary Wave 21 targets:
- `src/tui_shell/app/remote_mutations.rs` (~215 LOC)
- `src/tui_shell/app/cmd_transfer/mod.rs` (~211 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

### B) Remote Mutations Decomposition
- [x] Split `src/tui_shell/app/remote_mutations.rs` by pin/approve/promote/release concern.
- [x] Preserve interactive fallbacks (wizard/modal) and command output/error behavior.

### C) Transfer Command Decomposition
- [x] Split `src/tui_shell/app/cmd_transfer/mod.rs` by publish and sync concern.
- [x] Preserve parsed argument defaults and login-token fallback behavior.

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Verification notes:
- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
