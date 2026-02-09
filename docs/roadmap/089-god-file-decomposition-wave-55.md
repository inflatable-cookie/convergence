# Phase 089: God-File Decomposition (Wave 55)

## Goal

Decompose remote root command definitions, local snap/resolution storage methods, and remote release/promotion operations into focused helper modules.

## Scope

Primary Wave 55 targets:
- `src/tui_shell/commands/root_defs/remote.rs` (~141 LOC)
- `src/store/snap_resolution.rs` (~139 LOC)
- `src/remote/operations/release_promotion_gc.rs` (~137 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/commands/root_defs/remote.rs` into focused command-section helpers.
- [x] Split `src/store/snap_resolution.rs` into focused snaps/resolutions/head helpers.
- [x] Split `src/remote/operations/release_promotion_gc.rs` into focused gc/releases/promotions helpers.
- [x] Preserve command lists and remote/store behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo test --lib` completed successfully (`15 passed, 0 failed`).
