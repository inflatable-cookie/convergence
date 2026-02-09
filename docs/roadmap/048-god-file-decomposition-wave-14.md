# Phase 048: God-File Decomposition (Wave 14)

## Goal

Continue reducing high-LOC wizard/command/server files while preserving UX behavior, API contracts, and command definitions.

## Scope

Primary Wave 14 targets:
- `src/tui_shell/wizard/fetch_flow.rs` (~268 LOC)
- `src/tui_shell/commands/root_defs.rs` (~268 LOC)
- `src/bin/converge_server/handlers_publications/bundles.rs` (~267 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Start with `fetch_flow.rs` and `root_defs.rs` (low-risk structural splits).
- Then tackle server `handlers_publications/bundles.rs` by endpoint concern.

### B) Fetch Wizard Decomposition
- [x] Split `src/tui_shell/wizard/fetch_flow.rs` by parse/transition/effect concerns.
- [x] Preserve fetch wizard prompts, defaults, and final command behavior.

Progress notes:
- Replaced `src/tui_shell/wizard/fetch_flow.rs` with module directory:
  - `src/tui_shell/wizard/fetch_flow/mod.rs`
  - `src/tui_shell/wizard/fetch_flow/flow.rs`
  - `src/tui_shell/wizard/fetch_flow/transitions.rs`
  - `src/tui_shell/wizard/fetch_flow/finish.rs`
- Preserved fetch wizard start flow, transition prompts, and final `cmd_fetch_impl` argument assembly.

### C) Root Command Definitions Decomposition
- [x] Split `src/tui_shell/commands/root_defs.rs` into grouped command family modules.
- [x] Preserve command names, aliases, usage, and help text.

Progress notes:
- Replaced `src/tui_shell/commands/root_defs.rs` with module directory:
  - `src/tui_shell/commands/root_defs/mod.rs`
  - `src/tui_shell/commands/root_defs/global_local.rs`
  - `src/tui_shell/commands/root_defs/remote.rs`
- Preserved exported function names (`global_command_defs`, `local_root_command_defs`, `remote_root_command_defs`, `root_command_defs`) and all command metadata text.

### D) Publication Bundles Handler Decomposition
- [ ] Split `src/bin/converge_server/handlers_publications/bundles.rs` by endpoint/helper concerns.
- [ ] Preserve route signatures and response payloads.

### E) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

Progress notes:
- Validation for fetch-flow + root-defs slices:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo test --lib` passed (`15 passed`, `0 failed`)
