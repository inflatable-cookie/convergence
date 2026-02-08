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

- [x] Split `src/bin/converge_server/handlers_repo.rs` into focused modules by concern (repo CRUD, membership, lanes/heads).
- [x] Keep route signatures and response payloads unchanged.
- [x] Reduce wildcard import dependence where direct sibling imports are possible.

Progress notes:
- Replaced monolithic repo handler file with module directory:
  - `src/bin/converge_server/handlers_repo/mod.rs`
  - `src/bin/converge_server/handlers_repo/repo_crud.rs`
  - `src/bin/converge_server/handlers_repo/members.rs`
  - `src/bin/converge_server/handlers_repo/lanes.rs`
  - `src/bin/converge_server/handlers_repo/lane_heads.rs`
- Kept route wiring behavior-compatible by preserving exported handler names used by `routes.rs`.
- Replaced wildcard re-exports in `handlers_repo/mod.rs` with explicit exported handler symbols.

### D) CLI Release Resolve Decomposition

- [x] Split `src/cli_exec/release_resolve.rs` into resolve planning, validation, and output/report helpers.
- [x] Keep CLI output ordering and error text behavior-compatible.
- [x] Add focused unit tests around extracted pure helpers.

Progress notes:
- Replaced `src/cli_exec/release_resolve.rs` with module directory:
  - `src/cli_exec/release_resolve/mod.rs`
  - `src/cli_exec/release_resolve/release_cmd.rs`
  - `src/cli_exec/release_resolve/resolve_init.rs`
  - `src/cli_exec/release_resolve/resolve_pick_clear_show.rs`
  - `src/cli_exec/release_resolve/resolve_apply_validate.rs`
- Preserved CLI command dispatch surface and output/error text in release/resolve flows while isolating command-group logic.
- Added unit coverage for extracted pure resolve-pick helper (`parse_pick_specifier`) including:
  - conflicting/ambiguous flag validation
  - missing required selector validation
  - out-of-range variant validation
  - accepted variant/key selector forms

### E) Verification and Hygiene

- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.
- [x] Update any impacted architecture/decision notes if boundaries change materially.

Progress notes:
- Current validation status after Wave 5 decomposition slices:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo nextest run` passed (`60 passed`, `0 skipped`)
- Decision-doc impact review: no architecture/decision doc updates required because this wave is decomposition-only with unchanged command/route semantics.

## Exit Criteria

- Primary Wave 5 targets are decomposed into thin orchestration entrypoints plus focused helpers.
- Imports/visibility are narrower or unchanged in safety.
- Validation checks pass and roadmap reflects delivered module boundaries.
