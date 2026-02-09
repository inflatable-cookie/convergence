# Phase 090: God-File Decomposition (Wave 56)

## Goal

Decompose remote command section definitions, GC workflow orchestration helpers, and remote member command parsing into focused modules.

## Scope

Primary Wave 56 targets:
- `src/tui_shell/commands/root_defs/remote/sections.rs` (~148 LOC)
- `src/bin/converge_server/handlers_gc/workflow.rs` (~146 LOC)
- `src/tui_shell/app/remote_members/member.rs` (~133 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/tui_shell/commands/root_defs/remote/sections.rs` into admin/browse/delivery section modules.
- [x] Split `src/bin/converge_server/handlers_gc/workflow.rs` into focused retention/sweep/report helpers.
- [x] Split `src/tui_shell/app/remote_members/member.rs` into prompt-first and legacy flag parsing helpers.
- [x] Preserve command behavior and server GC semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` completed successfully.
- `cargo clippy --all-targets -- -D warnings` completed successfully.
- `cargo test --lib` completed successfully (`15 passed, 0 failed`).
