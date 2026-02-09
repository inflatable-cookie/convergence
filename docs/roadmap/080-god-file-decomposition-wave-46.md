# Phase 080: God-File Decomposition (Wave 46)

## Goal

Decompose CLI command dispatch and server token handlers into focused helper modules.

## Scope

Primary Wave 46 targets:
- `src/cli_exec.rs` (~162 LOC)
- `src/bin/converge_server/handlers_identity/tokens.rs` (~161 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/cli_exec.rs` into focused command dispatch and workspace-discovery helpers.
- [x] Split `src/bin/converge_server/handlers_identity/tokens.rs` into focused types/mint/persist/revoke helpers.
- [x] Preserve CLI command semantics and token handler response behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo nextest run -E 'kind(lib)'` completed successfully (`15 passed, 0 failed`).
