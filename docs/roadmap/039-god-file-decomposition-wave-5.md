# Phase 039: God-File Decomposition (Wave 5)

## Goal

Continue reducing maintenance and change-risk by decomposing remaining high-LOC, high-churn files identified after Wave 4.

## Scope

Primary Wave 5 targets (post-Wave-4 LOC snapshot):
- `src/tui_shell/app/cmd_transfer.rs` (~434 LOC)
- `src/bin/converge_server/handlers_repo.rs` (~423 LOC)
- `src/cli_exec/release_resolve.rs` (~408 LOC)

Secondary follow-ons:
- `src/bin/converge_server/persistence.rs` (~381 LOC)
- `src/remote/operations.rs` (~380 LOC)
- `src/tui_shell/app/cmd_gate_graph.rs` (~373 LOC)

## Non-Goals

- behavior/UX changes beyond decomposition-safe refactors
- API/protocol redesign
- unrelated performance work

## Tasks

### A) Baseline and Boundaries

- [x] Capture refreshed LOC snapshot for Wave 5 targets.
- [x] Define decomposition order and risk notes.
- [x] Document module boundary intent for transfer commands, server handlers, and release-resolve flow.

Progress notes:
- Order/risk:
  - Start with `cmd_transfer` (TUI command parsing split; low external coupling, user-visible CLI syntax risk).
  - Continue with `handlers_repo` (higher fan-in to auth/persistence/object graph; preserve HTTP behavior exactly).
  - Finish primary with `release_resolve` (CLI resolution semantics; preserve output/exit behavior).
- Boundary intent:
  - Keep top-level files as orchestration entrypoints.
  - Move argument parsing/validation into pure helpers.
  - Isolate mode-specific fetch/selection behavior from transfer execution.
  - Avoid widening visibility; prefer module-private and `pub(super)` exports only.

### B) TUI Transfer Command Decomposition

- [x] Split `src/tui_shell/app/cmd_transfer.rs` into orchestration + parsing helpers.
- [x] Separate mode-specific fetch helpers from publish/sync execution paths.
- [x] Keep behavior-equivalent argument parsing for flag and flagless forms.

Progress notes:
- Replaced monolithic `cmd_transfer.rs` with module directory:
  - `src/tui_shell/app/cmd_transfer/mod.rs` (publish/sync orchestration and execution)
  - `src/tui_shell/app/cmd_transfer/publish_args.rs` (publish argument parsing)
  - `src/tui_shell/app/cmd_transfer/sync_args.rs` (sync argument parsing)
  - `src/tui_shell/app/cmd_transfer/mode_fetch.rs` (lanes/releases fetch-mode command helpers)
- Preserved existing usage/error strings and command semantics for flag and flagless forms.

### C) Server Repo Handler Decomposition

- [ ] Split `src/bin/converge_server/handlers_repo.rs` into focused modules by concern (repo CRUD, membership, lanes/heads).
- [ ] Keep route signatures and response payloads unchanged.
- [ ] Reduce wildcard import dependence where direct sibling imports are possible.

### D) CLI Release Resolve Decomposition

- [ ] Split `src/cli_exec/release_resolve.rs` into resolve planning, validation, and output/report helpers.
- [ ] Keep CLI output ordering and error text behavior-compatible.
- [ ] Add focused unit tests around extracted pure helpers.

### E) Verification and Hygiene

- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run`.
- [ ] Update any impacted architecture/decision notes if boundaries change materially.

## Exit Criteria

- Primary Wave 5 targets are decomposed into thin orchestration entrypoints plus focused helpers.
- Imports/visibility are narrower or unchanged in safety.
- Validation checks pass and roadmap reflects delivered module boundaries.
