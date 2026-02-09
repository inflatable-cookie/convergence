# Phase 052: God-File Decomposition (Wave 18)

## Goal

Continue reducing high-LOC handlers/views by separating identity HTTP concerns and snap-history rendering concerns into focused modules with no behavior drift.

## Scope

Primary Wave 18 targets:
- `src/bin/converge_server/handlers_identity.rs` (~232 LOC)
- `src/tui_shell/views/snaps.rs` (~238 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Split identity handlers by profile/users/token concerns.
- Split snaps view by row/detail rendering concerns while keeping selection logic stable.

### B) Identity Handler Decomposition
- [x] Split `src/bin/converge_server/handlers_identity.rs` into focused submodules.
- [x] Preserve auth checks, persistence behavior, and response payloads.

### C) Snaps View Decomposition
- [x] Split `src/tui_shell/views/snaps.rs` by list row composition and details-pane composition.
- [x] Preserve row ordering, selection mapping, and Enter-action context hints.

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Verification notes:
- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run` stalled in this environment; fallback `cargo test --lib` passed (`15 passed, 0 failed`).
