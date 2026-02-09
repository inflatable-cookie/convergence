# Phase 085: God-File Decomposition (Wave 51)

## Goal

Decompose resolve apply/validate execution, publish upload object transfer helpers, and sync wizard transitions into focused modules.

## Scope

Primary Wave 51 targets:
- `src/cli_exec/release_resolve/resolve_apply_validate.rs` (~151 LOC)
- `src/remote/transfer/publish/uploads.rs` (~150 LOC)
- `src/tui_shell/wizard/publish_sync_flow/sync.rs` (~149 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/cli_exec/release_resolve/resolve_apply_validate.rs` into apply/validate focused helpers.
- [x] Split `src/remote/transfer/publish/uploads.rs` into focused upload object-kind helpers.
- [x] Split `src/tui_shell/wizard/publish_sync_flow/sync.rs` into focused wizard start/transition/finish helpers.
- [x] Preserve resolve, publish upload, and wizard behavior.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo nextest run -E 'kind(lib)'` stalled in this environment after build output.
- Fallback `cargo test --lib` completed successfully (`15 passed, 0 failed`).
