# Phase 101: God-File Decomposition (Wave 67)

## Goal

Decompose rename-focused status tests and delivery CLI status/sync flows into focused helper modules.

## Scope

Primary Wave 67 targets:
- `src/tui_shell/status/rename_tests.rs` (~250 LOC)
- `src/cli_exec/delivery/moderation_status/status.rs` (~119 LOC)
- `src/cli_exec/delivery/publish_sync.rs` (~113 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/status/rename_tests.rs` into shared fixtures and per-scenario tests.
- [x] Split `src/cli_exec/delivery/moderation_status/status.rs` into JSON/text/report helpers.
- [x] Split `src/cli_exec/delivery/publish_sync.rs` into publish/sync/lanes handlers.
- [x] Preserve test assertions and CLI output semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built tests but stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
