# Phase 094: God-File Decomposition (Wave 60)

## Goal

Decompose gate-graph text input handling, remote bundle operations, and status summary utilities into focused helper modules.

## Scope

Primary Wave 60 targets:
- `src/tui_shell/app/cmd_gate_graph/text_input.rs` (~130 LOC)
- `src/remote/operations/bundle_ops.rs` (~131 LOC)
- `src/tui_shell/status/summary_utils.rs` (~131 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/app/cmd_gate_graph/text_input.rs` into focused add/edit handlers.
- [x] Split `src/remote/operations/bundle_ops.rs` into focused bundles/pins/approval handlers.
- [x] Split `src/tui_shell/status/summary_utils.rs` into focused parsing/extraction/similarity helpers.
- [x] Preserve current command behavior and utility semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` built test binaries, but the runner stalled with no further output in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
