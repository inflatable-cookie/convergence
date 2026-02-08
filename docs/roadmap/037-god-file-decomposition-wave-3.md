# Phase 037: God-File Decomposition (Wave 3)

## Goal

Continue reducing maintenance risk by decomposing the next set of highest-churn, highest-size files into focused modules with explicit ownership and stable boundaries.

## Scope

Primary Wave 3 targets (post-Phase-036 inventory):
- `src/tui_shell/status/tree_diff.rs` (~960 LOC)
- `src/main.rs` (~651 LOC)
- `src/bin/converge_server/object_graph.rs` (~621 LOC)
- `src/tui_shell/commands.rs` (~537 LOC)
- `src/bin/converge_server/handlers_publications.rs` (~536 LOC)
- `src/cli_exec/delivery.rs` (~522 LOC)
- `src/tui_shell/app/local_snaps.rs` (~512 LOC)

Secondary candidates (as needed during execution):
- `src/tui_shell/wizard/login_bootstrap_flow.rs` (~495 LOC)
- `src/tui_shell/views/root.rs` (~492 LOC)
- `src/tui_shell/app/cmd_dispatch.rs` (~456 LOC)
- `src/tui_shell/modal.rs` (~450 LOC)
- `src/bin/converge-server.rs` (~449 LOC)

## Non-Goals

- behavior or UX changes beyond decomposition-safe refactors
- protocol/schema redesign
- unrelated performance work

## Tasks

### A) Baseline, Order, and Boundaries

- [x] Capture refreshed LOC + ownership inventory for Wave 3 targets.
- [x] Define decomposition order with explicit risk notes (stateful flows first, pure helpers second).
- [x] Document module boundary rules for CLI, server handlers, and TUI status diff flows.

Progress notes:
- Refreshed LOC snapshot:
  - `src/tui_shell/status/tree_diff.rs` (960)
  - `src/main.rs` (651)
  - `src/bin/converge_server/object_graph.rs` (621)
  - `src/tui_shell/commands.rs` (537)
  - `src/bin/converge_server/handlers_publications.rs` (536)
  - `src/cli_exec/delivery.rs` (522)
  - `src/tui_shell/app/local_snaps.rs` (512)
- Decomposition order/risk:
  - Start with `status/tree_diff.rs` (high complexity, pure computation paths available for safe extraction first).
  - Then `local_snaps.rs` and `commands.rs` (TUI command orchestration with bounded behavior surface).
  - Then `main.rs`/`delivery.rs` (CLI wiring and execution separation).
  - Then server modules (`object_graph.rs`, `handlers_publications.rs`) with request/retention boundaries.
- Boundary rules:
  - Parser modules: string/flag decoding only, no workspace/store/server side effects.
  - Operation modules: apply validated inputs and call domain APIs; no rendering concerns.
  - View/format modules: presentation-only and deterministic from view-model inputs.
  - Server handler modules: parse/validate/request-map separated from storage mutation and traversal.

### B) TUI Status and Local Command Decomposition

- [x] Split `src/tui_shell/status/tree_diff.rs` into focused modules (leaf collection, identity mapping, rename heuristics, rendering payload assembly).
- [x] Split `src/tui_shell/app/local_snaps.rs` into focused mode handlers (filtering, message edits, restore/revert actions, view refresh).
- [x] Keep top-level status/local command modules as orchestration-only entry points.

Progress notes:
- Started `tree_diff.rs` decomposition by extracting blob/recipe IO helpers into `src/tui_shell/status/rename_io.rs` and wiring `tree_diff.rs` to consume these helpers.
- Continued `tree_diff.rs` decomposition by extracting rename detection and consumed-path tracking into `src/tui_shell/status/rename_match.rs`.
- Continued `tree_diff.rs` decomposition by extracting manifest tree traversal and status-delta walk helpers into `src/tui_shell/status/tree_walk.rs`.
- Continued `tree_diff.rs` decomposition by extracting remote status/dashboard builders into `src/tui_shell/status/remote_status.rs`.
- Completed `tree_diff.rs` decomposition by extracting local status line assembly into `src/tui_shell/status/local_status.rs`, reducing `tree_diff.rs` to thin diff orchestration.
- Started `local_snaps.rs` decomposition by extracting snaps filter and clear-filter handlers into `src/tui_shell/app/local_snaps_filter.rs`.
- Continued `local_snaps.rs` decomposition by extracting snap message edit/clear handling into `src/tui_shell/app/local_snaps_message.rs`.
- Continued `local_snaps.rs` decomposition by extracting snaps list/open flow into `src/tui_shell/app/local_snaps_open.rs`.
- Continued `local_snaps.rs` decomposition by extracting revert/restore flows into `src/tui_shell/app/local_snaps_restore.rs`.
- Continued `local_snaps.rs` decomposition by extracting unsnap flow into `src/tui_shell/app/local_snaps_unsnap.rs`.
- Completed `local_snaps.rs` decomposition by extracting the remaining snap handler into `src/tui_shell/app/local_snaps_snap.rs` and removing the legacy `local_snaps.rs` module file.

### C) CLI Surface Decomposition

- [x] Split `src/main.rs` into command registration/composition modules and execution wiring.
- [x] Split `src/tui_shell/commands.rs` into grouped command catalogs by mode/domain.
- [x] Split `src/cli_exec/delivery.rs` into publish/promote/release-specific execution modules.

Progress notes:
- Started `commands.rs` decomposition by extracting mode-specific command catalogs (`snaps`, `inbox`, `bundles`, `releases`, `lanes`, `gate-graph`, `superpositions`) into `src/tui_shell/commands/mode_defs.rs`, with `commands.rs` re-exporting the same API.
- Completed `commands.rs` decomposition by extracting root/global/auth command catalogs into `src/tui_shell/commands/root_defs.rs`, leaving `commands.rs` as thin module composition/re-export.
- Started `main.rs` decomposition by extracting subordinate CLI subcommand enums into `src/cli_subcommands.rs`, keeping top-level `Commands` and runtime entry flow in `main.rs`.
- Continued `main.rs` decomposition by extracting the top-level `Commands` enum into `src/cli_commands.rs` and re-exporting command types from `main.rs`.
- Completed `main.rs` decomposition by extracting runtime parsing/dispatch and remote token helpers into `src/cli_runtime.rs`, leaving `main.rs` as thin module composition + process entry.
- Completed `cli_exec/delivery.rs` decomposition by splitting handlers into focused modules:
  - `src/cli_exec/delivery/publish_sync.rs`
  - `src/cli_exec/delivery/transfer.rs`
  - `src/cli_exec/delivery/moderation_status.rs`
  with `src/cli_exec/delivery/mod.rs` as composition/re-export layer.

### D) Server Surface Decomposition

- [x] Split `src/bin/converge_server/object_graph.rs` into graph traversal, retention policy, and prune execution helpers.
- [x] Split `src/bin/converge_server/handlers_publications.rs` into request parsing, validation, and handler core modules.
- [x] Minimize cross-module visibility (`pub` -> `pub(crate)`/private where possible) after extraction.

Progress notes:
- Completed `object_graph.rs` decomposition by splitting storage/validation, traversal checks, and merge/coalesce logic into:
  - `src/bin/converge_server/object_graph/store.rs`
  - `src/bin/converge_server/object_graph/traversal.rs`
  - `src/bin/converge_server/object_graph/merge.rs`
  with `src/bin/converge_server/object_graph/mod.rs` as a thin composition/re-export layer.
- Completed `handlers_publications.rs` decomposition into focused modules:
  - `src/bin/converge_server/handlers_publications/missing_objects.rs`
  - `src/bin/converge_server/handlers_publications/publications.rs`
  - `src/bin/converge_server/handlers_publications/bundles.rs`
  - `src/bin/converge_server/handlers_publications/pins.rs`
  with `src/bin/converge_server/handlers_publications/mod.rs` as a thin composition/re-export layer.
- Tightened server decomposition visibility by replacing submodule re-exports with wrapper entrypoints in `handlers_publications/mod.rs` and narrowing child module items to `pub(super)`.

### E) Regression and Verification

- [x] Add focused tests for newly extracted boundaries where current coverage is indirect.
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.

Progress notes:
- Added focused unit tests in `src/bin/converge_server/object_graph/merge.rs` for `compute_promotability`, covering success, superposition rejection, and combined-reason rejection paths.
- Ran `cargo fmt` after the latest status/local-snaps extraction slices.
- Ran `cargo clippy --all-targets -- -D warnings` with no warnings/errors.
- Ran `cargo nextest run` and passed all tests (51 passed, 0 skipped).

## Exit Criteria

- Each primary Wave 3 target is reduced to a thin orchestration layer.
- Ownership and module boundaries are explicit in code and roadmap notes.
- Full lint/test suite passes after decomposition.
