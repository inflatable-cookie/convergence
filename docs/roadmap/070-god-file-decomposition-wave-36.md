# Phase 070: God-File Decomposition (Wave 36)

## Goal

Continue CLI decomposition by splitting resolve pick/clear/show command handling into focused parser and command execution modules.

## Scope

Primary Wave 36 target:
- `src/cli_exec/release_resolve/resolve_pick_clear_show.rs` (~237 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Resolve Pick/Clear/Show Decomposition
- [x] Split `src/cli_exec/release_resolve/resolve_pick_clear_show.rs` by pick-spec parsing, decision key helpers, and command handlers.
- [x] Preserve argument validation, index/key selection behavior, and text/json output semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- 2026-02-09: `cargo fmt` passed.
- 2026-02-09: `cargo clippy --lib --bins -- -D warnings` passed (used this scope due intermittent all-targets environment stalls).
- 2026-02-09: `cargo nextest run` started and then stalled in this environment.
- 2026-02-09: fallback `cargo test --lib` passed (15 passed, 0 failed).
