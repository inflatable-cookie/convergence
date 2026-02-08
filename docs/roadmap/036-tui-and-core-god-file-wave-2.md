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
- Started `app.rs` decomposition by extracting mode-scoped command catalog composition into `src/tui_shell/app/mode_commands.rs`.
- Continued `app.rs` decomposition by extracting root-context switching and remote-identity refresh helpers into `src/tui_shell/app/root_context.rs`.
- Continued `app.rs` decomposition by extracting modal and output/log helper methods into `src/tui_shell/app/modal_output.rs`.
- Continued `app.rs` decomposition by extracting view-stack navigation helpers (`mode`, `view`, `current_view`, `push_view`, `pop_mode`, `prompt`) into `src/tui_shell/app/view_nav.rs`.
- Continued `app.rs` decomposition by extracting root dashboard refresh/state assembly into `src/tui_shell/app/root_refresh.rs`.
- Continued `app.rs` decomposition by extracting lifecycle/workspace bootstrap helpers (`load`, `require_workspace`) into `src/tui_shell/app/lifecycle.rs`.
- Continued `app.rs` decomposition by extracting mode-sensitive command availability filtering into `src/tui_shell/app/command_availability.rs`.
- Continued `cmd_remote_views.rs` decomposition by extracting inbox/bundles list builders (`open_inbox_view`, `open_bundles_view`) into `src/tui_shell/app/remote_list_views.rs`.
- Continued `cmd_remote_views.rs` decomposition by extracting fetch argument parsing/validation into `src/tui_shell/app/remote_fetch_parse.rs`.
- Continued `cmd_remote_views.rs` decomposition by extracting member and lane-member command handlers into `src/tui_shell/app/remote_members.rs`.
- Continued `cmd_remote_views.rs` decomposition by extracting shared scope/gate/filter/limit parsing for inbox/bundles into `src/tui_shell/app/remote_scope_query_parse.rs`.
- Started `cmd_local.rs` decomposition by extracting snaps-mode command handlers into `src/tui_shell/app/local_snaps.rs`.
- Continued `cmd_local.rs` decomposition by extracting local maintenance commands (`show`, `restore`, `move`, `gc`) into `src/tui_shell/app/local_maintenance.rs`.
- Continued `cmd_local.rs` decomposition by extracting remote config/client helpers into `src/tui_shell/app/remote_access.rs`.
- Continued `cmd_local.rs` decomposition by extracting informational helpers (`cmd_help`, `cmd_status`) into `src/tui_shell/app/local_info.rs`.
- Completed `cmd_local.rs` decomposition by extracting bootstrap helpers (`cmd_init`, `cmd_snap`) into `src/tui_shell/app/local_bootstrap.rs` and removing the legacy `cmd_local.rs` file.
- Started `cmd_remote_actions.rs` decomposition by extracting argument parsing for bundle/pin/approve/promote/release/superpositions into `src/tui_shell/app/remote_action_parse.rs`.
- Continued `cmd_remote_actions.rs` decomposition by extracting superpositions command orchestration into `src/tui_shell/app/remote_superpositions.rs`.
- Continued `cmd_remote_actions.rs` decomposition by extracting mutation flows (`cmd_pin`, `cmd_approve`, `cmd_promote`, `cmd_release`) into `src/tui_shell/app/remote_mutations.rs`.
- Completed `cmd_remote_actions.rs` decomposition by extracting bundle/pins handlers into `src/tui_shell/app/remote_bundle_ops.rs` and removing the legacy `cmd_remote_actions.rs` file.
- Started `cmd_settings.rs` decomposition by extracting settings snapshot/open/refresh helpers into `src/tui_shell/app/settings_overview.rs`.
- Continued `cmd_settings.rs` decomposition by extracting `cmd_chunking` and `cmd_retention` into `src/tui_shell/app/settings_chunking.rs` and `src/tui_shell/app/settings_retention.rs`.
- Completed `cmd_settings.rs` decomposition by extracting settings mode action handling into `src/tui_shell/app/settings_do_mode.rs` and removing the legacy `cmd_settings.rs` file.
- Continued `cmd_remote_views.rs` decomposition by extracting lanes/releases view loaders into `src/tui_shell/app/remote_lane_release_views.rs`.
- Started `workspace.rs` decomposition by extracting GC reachability traversal helpers into `src/workspace/gc.rs` and wiring `workspace.rs` through module imports.
- Continued `workspace.rs` decomposition by extracting restore/materialization filesystem helpers into `src/workspace/materialize_fs.rs`.
- Continued `workspace.rs` decomposition by extracting chunking policy constants/type/config parsing into `src/workspace/chunking.rs`.
- Continued `workspace.rs` decomposition by extracting chunked file read/hash and recipe persistence helpers into `src/workspace/chunk_io.rs`.

### C) Core Workspace/Store Decomposition

- [x] Split `src/workspace.rs` into modules for workspace lifecycle, snap creation, restore/diff orchestration, and metadata helpers.
- [x] Split `src/store.rs` into modules for object CRUD, integrity checks, and traversal/query helpers.
- [ ] Minimize cross-module visibility (`pub` -> `pub(super)`/private where possible).

Progress notes:
- Continued `workspace.rs` decomposition by extracting manifest scan/build and filesystem ordering helpers into `src/workspace/manifest_scan.rs`.
- Continued `workspace.rs` decomposition by extracting workspace path move/rename operations into `src/workspace/path_ops.rs`.
- Continued `workspace.rs` decomposition by moving retention/garbage-collection orchestration (`GcReport`, `gc_local`) into `src/workspace/gc.rs`.
- Started `store.rs` decomposition by extracting object traversal/query helpers (`list_blob_ids`, `list_manifest_ids`, `list_recipe_ids`, `delete_blob`, `delete_manifest`, `delete_recipe`) into `src/store/traversal.rs`.
- Continued `store.rs` decomposition by extracting workspace-state metadata helpers (lane sync, remote token, last published scope/gate metadata) into `src/store/state_meta.rs`.
- Continued `store.rs` decomposition by extracting blob/manifest/recipe object CRUD + integrity checks into `src/store/object_crud.rs`.
- Continued `store.rs` decomposition by extracting snap/resolution persistence and HEAD helpers into `src/store/snap_resolution.rs`.

### D) Regression and Verification

- [ ] Add focused tests for extracted wizard/status/workspace/store boundaries where coverage is currently indirect.
- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run`.

## Exit Criteria

- All target god files are reduced to thin orchestration/composition layers with clear module ownership.
- Extracted modules have explicit boundaries and minimal visibility.
- Existing behavior remains stable under full test suite.
