# Phase 046: God-File Decomposition (Wave 12)

## Goal

Continue reducing server and TUI high-LOC modules while preserving route contracts and CLI/TUI behavior.

## Scope

Primary Wave 12 targets:
- `src/bin/converge_server/handlers_gc.rs` (~302 LOC)
- `src/bin/converge_server/object_graph/traversal.rs` (~284 LOC)
- `src/tui_shell/app/default_actions.rs` (~283 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Start with `handlers_gc.rs` (endpoint clusters are separable).
- Continue with object graph traversal and then TUI default actions.

### B) GC Handler Decomposition
- [x] Split `src/bin/converge_server/handlers_gc.rs` by endpoint concern (gc runs vs pin operations).
- [x] Preserve route signatures and JSON response contracts.

Progress notes:
- Replaced `src/bin/converge_server/handlers_gc.rs` with module directory:
  - `src/bin/converge_server/handlers_gc/mod.rs`
  - `src/bin/converge_server/handlers_gc/sweep.rs`
- Moved filesystem sweep helper into `sweep.rs` and kept `gc_repo` endpoint contract and response shape unchanged.
- Updated `src/bin/converge-server.rs` handler module path to `handlers_gc/mod.rs`.

### C) Object Graph Traversal Decomposition
- [x] Split `src/bin/converge_server/object_graph/traversal.rs` by traversal and helper concerns.
- [x] Preserve traversal results and error behavior.

Progress notes:
- Replaced `src/bin/converge_server/object_graph/traversal.rs` with module directory:
  - `src/bin/converge_server/object_graph/traversal/mod.rs`
  - `src/bin/converge_server/object_graph/traversal/collect.rs`
  - `src/bin/converge_server/object_graph/traversal/validate.rs`
  - `src/bin/converge_server/object_graph/traversal/superpositions.rs`
- Preserved public traversal API re-exports used by object graph call sites.

### D) TUI Default Actions Decomposition
- [ ] Split `src/tui_shell/app/default_actions.rs` by mode/intent concerns.
- [ ] Preserve key bindings and actions.

### E) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Progress notes:
- Validation for `handlers_gc` decomposition:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - Targeted fallback tests passed:
    - `cargo test server_gc_retention -- --nocapture`
    - `cargo test server_gc_release_retention -- --nocapture`
- Validation for `object_graph/traversal` decomposition:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - Targeted server contract test passed:
    - `cargo test server_api_contract -- --nocapture`
