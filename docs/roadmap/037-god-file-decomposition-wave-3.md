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

- [ ] Split `src/tui_shell/status/tree_diff.rs` into focused modules (leaf collection, identity mapping, rename heuristics, rendering payload assembly).
- [x] Split `src/tui_shell/app/local_snaps.rs` into focused mode handlers (filtering, message edits, restore/revert actions, view refresh).
- [ ] Keep top-level status/local command modules as orchestration-only entry points.

Progress notes:
- Started `tree_diff.rs` decomposition by extracting blob/recipe IO helpers into `src/tui_shell/status/rename_io.rs` and wiring `tree_diff.rs` to consume these helpers.
- Continued `tree_diff.rs` decomposition by extracting rename detection and consumed-path tracking into `src/tui_shell/status/rename_match.rs`.
- Continued `tree_diff.rs` decomposition by extracting manifest tree traversal and status-delta walk helpers into `src/tui_shell/status/tree_walk.rs`.
- Started `local_snaps.rs` decomposition by extracting snaps filter and clear-filter handlers into `src/tui_shell/app/local_snaps_filter.rs`.
- Continued `local_snaps.rs` decomposition by extracting snap message edit/clear handling into `src/tui_shell/app/local_snaps_message.rs`.
- Continued `local_snaps.rs` decomposition by extracting snaps list/open flow into `src/tui_shell/app/local_snaps_open.rs`.
- Continued `local_snaps.rs` decomposition by extracting revert/restore flows into `src/tui_shell/app/local_snaps_restore.rs`.
- Continued `local_snaps.rs` decomposition by extracting unsnap flow into `src/tui_shell/app/local_snaps_unsnap.rs`.
- Completed `local_snaps.rs` decomposition by extracting the remaining snap handler into `src/tui_shell/app/local_snaps_snap.rs` and removing the legacy `local_snaps.rs` module file.

### C) CLI Surface Decomposition

- [ ] Split `src/main.rs` into command registration/composition modules and execution wiring.
- [ ] Split `src/tui_shell/commands.rs` into grouped command catalogs by mode/domain.
- [ ] Split `src/cli_exec/delivery.rs` into publish/promote/release-specific execution modules.

### D) Server Surface Decomposition

- [ ] Split `src/bin/converge_server/object_graph.rs` into graph traversal, retention policy, and prune execution helpers.
- [ ] Split `src/bin/converge_server/handlers_publications.rs` into request parsing, validation, and handler core modules.
- [ ] Minimize cross-module visibility (`pub` -> `pub(crate)`/private where possible) after extraction.

### E) Regression and Verification

- [ ] Add focused tests for newly extracted boundaries where current coverage is indirect.
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.

Progress notes:
- Ran `cargo fmt` after the latest status/local-snaps extraction slices.
- Ran `cargo clippy --all-targets -- -D warnings` with no warnings/errors.
- Ran `cargo nextest run` and passed all tests (51 passed, 0 skipped).

## Exit Criteria

- Each primary Wave 3 target is reduced to a thin orchestration layer.
- Ownership and module boundaries are explicit in code and roadmap notes.
- Full lint/test suite passes after decomposition.
