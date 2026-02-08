# Phase 036: TUI and Core God-File Decomposition (Wave 2)

## Goal

Reduce maintenance risk in the largest remaining files by splitting TUI interaction logic and core workspace/store orchestration into focused modules with explicit ownership.

## Scope

This phase targets high-line-count files that still mix multiple responsibilities:
- `src/tui_shell/wizard.rs` (~2.1k LOC)
- `src/tui_shell/status.rs` (~1.6k LOC)
- `src/tui_shell/app.rs` (~1.0k LOC)
- `src/workspace.rs` (~0.9k LOC)
- `src/store.rs` (~0.6k LOC)

## Non-Goals

- changing CLI/TUI behavior or UX semantics
- protocol or persistence format redesign
- introducing new features unrelated to decomposition

## Tasks

### A) Baseline Inventory and Boundaries

- [x] Capture current largest-file inventory and prioritize split order by risk/complexity.
- [x] Add module ownership notes for each target file (what stays, what moves, why).
- [x] Define hard boundaries for extracted modules (data types vs orchestration vs IO).

Progress notes:
- Current top candidates from scan:
  - `src/tui_shell/wizard.rs` (2112 LOC)
  - `src/tui_shell/status.rs` (1619 LOC)
  - `src/tui_shell/app.rs` (971 LOC)
  - `src/workspace.rs` (921 LOC)
  - `src/store.rs` (609 LOC)
- Ownership notes (initial):
  - `src/tui_shell/wizard.rs`: keep shell-facing `Wizard` composition/public API at top-level; move step-specific render/input logic into per-step modules.
  - `src/tui_shell/status.rs`: keep status entry API and shared output structs centralized; move snapshot diff/stat analysis and text rendering helpers into separate modules.
  - `src/tui_shell/app.rs`: keep `App` state container and external event loop hooks centralized; move command-mode-specific transitions/helpers into `src/tui_shell/app/*`.
  - `src/workspace.rs`: keep `Workspace` facade and root path/store ownership centralized; move snap/restore/diff internals into helper modules.
  - `src/store.rs`: keep `LocalStore` facade centralized; move object-type-specific load/save/verify helpers into typed submodules.
- Boundary rules:
  - Orchestration modules can call data/IO modules, but data/IO modules cannot depend on TUI rendering or command parsing layers.
  - Render modules are pure presentation (input: view model, output: UI text/widgets) and do not mutate workspace/store state directly.
  - IO modules are responsible for disk/network interactions; model modules stay deterministic and side-effect-free.
  - Extracted helper modules default to private visibility, promoting items only when consumed by another sibling module.

### B) TUI Wizard/Status Decomposition

- [ ] Split `src/tui_shell/wizard.rs` into focused modules (state model, rendering, input transitions, command execution bridge).
- [ ] Split `src/tui_shell/status.rs` into focused modules (snapshot modeling, diff analysis, formatting/presentation helpers).
- [ ] Keep top-level files as thin composition/entry modules.

Progress notes:
- Started `wizard.rs` decomposition by extracting filesystem glob/path search helpers into `src/tui_shell/wizard/move_glob.rs`; `wizard.rs` now imports `move_glob::glob_search` instead of owning low-level file-walk logic.
- Continued `wizard.rs` decomposition by extracting wizard DTO/enums into `src/tui_shell/wizard/types.rs`; `wizard.rs` now acts more as behavior/orchestration over typed wizard state.
- Continued `wizard.rs` decomposition by extracting move-flow state transitions (`move_wizard_from` / `move_wizard_to`) into `src/tui_shell/wizard/move_flow.rs`.
- Continued `wizard.rs` decomposition by extracting fetch-flow transitions (`start_fetch_wizard` / `continue_fetch_wizard` / `finish_fetch_wizard`) into `src/tui_shell/wizard/fetch_flow.rs`.
- Continued `wizard.rs` decomposition by extracting browse-flow transitions (`start_browse_wizard` / `continue_browse_wizard` / `finish_browse_wizard`) into `src/tui_shell/wizard/browse_flow.rs`.
- Continued `wizard.rs` decomposition by extracting member and lane-member flows into `src/tui_shell/wizard/member_flow.rs`.
- Continued `wizard.rs` decomposition by extracting release/pin/promote flow handlers into `src/tui_shell/wizard/release_ops_flow.rs`.
- Continued `wizard.rs` decomposition by extracting publish/sync flow handlers into `src/tui_shell/wizard/publish_sync_flow.rs`.
- Continued `wizard.rs` decomposition by extracting login/bootstrap flow handlers into `src/tui_shell/wizard/login_bootstrap_flow.rs`.
- Started `status.rs` decomposition by extracting summary/parsing helpers (`ChangeSummary`, baseline/change-key parsing, Jaccard + blank-line normalization) into `src/tui_shell/status/summary_utils.rs` and re-exporting from `status.rs`.
- Continued `status.rs` decomposition by extracting rename/diff helper types and blob-rename scoring/default chunk-size helpers into `src/tui_shell/status/rename_helpers.rs`.
- Continued `status.rs` decomposition by extracting manifest identity traversal helpers into `src/tui_shell/status/identity_collect.rs`.
- Continued `status.rs` decomposition by extracting line-delta formatting and Myers/UTF-8 text delta helpers into `src/tui_shell/status/text_delta.rs`.
- Continued `status.rs` decomposition by moving recipe-rename scoring thresholds/comparison helpers into `src/tui_shell/status/rename_helpers.rs`.
- Continued `status.rs` decomposition by moving core diff/rename orchestration and dashboard/remote-status assembly into `src/tui_shell/status/tree_diff.rs`, keeping `status.rs` as module composition and shared exports.
- Started `workspace.rs` decomposition by extracting GC reachability traversal helpers into `src/workspace/gc.rs` and wiring `workspace.rs` through module imports.

### C) Core Workspace/Store Decomposition

- [ ] Split `src/workspace.rs` into modules for workspace lifecycle, snap creation, restore/diff orchestration, and metadata helpers.
- [ ] Split `src/store.rs` into modules for object CRUD, integrity checks, and traversal/query helpers.
- [ ] Minimize cross-module visibility (`pub` -> `pub(super)`/private where possible).

### D) Regression and Verification

- [ ] Add focused tests for extracted wizard/status/workspace/store boundaries where coverage is currently indirect.
- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run`.

## Exit Criteria

- All target god files are reduced to thin orchestration/composition layers with clear module ownership.
- Extracted modules have explicit boundaries and minimal visibility.
- Existing behavior remains stable under full test suite.
