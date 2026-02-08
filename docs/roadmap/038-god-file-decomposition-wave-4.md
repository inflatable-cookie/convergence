# Phase 038: God-File Decomposition (Wave 4)

## Goal

Continue reducing maintenance risk by decomposing the next largest, highest-churn files into focused modules with explicit ownership and thin orchestration entrypoints.

## Scope

Primary Wave 4 targets (current LOC snapshot):
- `src/tui_shell/wizard/login_bootstrap_flow.rs` (~495 LOC)
- `src/tui_shell/views/root.rs` (~492 LOC)
- `src/tui_shell/app.rs` (~483 LOC)
- `src/tui_shell/app/cmd_dispatch.rs` (~456 LOC)
- `src/tui_shell/modal.rs` (~450 LOC)
- `src/bin/converge-server.rs` (~449 LOC)

Secondary candidates:
- `src/tui_shell/app/cmd_transfer.rs` (~434 LOC)
- `src/bin/converge_server/handlers_repo.rs` (~423 LOC)
- `src/cli_exec/release_resolve.rs` (~408 LOC)

## Non-Goals

- behavior or UX changes beyond decomposition-safe refactors
- protocol/schema redesign
- unrelated performance work

## Tasks

### A) Baseline, Order, and Boundaries

- [x] Capture refreshed LOC + ownership inventory for Wave 4 targets.
- [x] Define decomposition order with explicit risk notes for wizard/view/app state flows.
- [x] Document module boundary rules for TUI wizard/view/controller modules and server entry composition.

Progress notes:
- Refreshed LOC snapshot captured from `src/` and ranked by file size.
- Decomposition order/risk:
  - Start with `login_bootstrap_flow.rs` (stateful, user-facing auth flow; extract pure validation and transition helpers first).
  - Then `views/root.rs` and `modal.rs` (rendering separation with low side-effect risk).
  - Then `app/cmd_dispatch.rs` and `app.rs` (controller orchestration, broader call graph).
  - Finish with `converge-server.rs` root entry split (high fan-in; preserve route wiring behavior exactly).
- Boundary rules:
  - Wizard modules: parse/validate/transition/effects split; only effect modules may touch IO/network.
  - View modules: deterministic rendering helpers from explicit view-model inputs; no state mutation.
  - App/controller modules: dispatch and coordination only; domain commands execute in dedicated command modules.
  - Server entry modules: runtime/bootstrap/state construction separated from route registration and handler modules.

### B) TUI Wizard and View Decomposition

- [x] Split `src/tui_shell/wizard/login_bootstrap_flow.rs` into focused stages (state transitions, validation, and effect execution).
- [x] Split `src/tui_shell/views/root.rs` into view-model assembly, layout selection, and section rendering helpers.
- [x] Keep top-level wizard/view modules as orchestration-only entry points.

Progress notes:
- Started `login_bootstrap_flow.rs` decomposition by extracting validation parsing helpers into `src/tui_shell/wizard/login_bootstrap_validate.rs` and wiring the flow to use them.
- Completed `login_bootstrap_flow.rs` staged split:
  - orchestration starts in `src/tui_shell/wizard/login_bootstrap_flow.rs`
  - transition handlers in `src/tui_shell/wizard/login_bootstrap_transitions.rs`
  - side-effect execution in `src/tui_shell/wizard/login_bootstrap_effects.rs`
- Started `views/root.rs` decomposition by extracting remote-dashboard rendering and line styling helpers into:
  - `src/tui_shell/views/root/render_remote.rs`
  - `src/tui_shell/views/root/style_line.rs`
- Continued `views/root.rs` decomposition by extracting local header/baseline rendering heuristics into:
  - `src/tui_shell/views/root/local_header.rs`
- Continued `views/root.rs` decomposition by extracting local refresh state updates and significance heuristics into:
  - `src/tui_shell/views/root/refresh_local.rs`

### C) TUI App Surface Decomposition

- [x] Split `src/tui_shell/app.rs` into app state construction, mode switching, and event-loop dispatch helpers.
- [x] Split `src/tui_shell/app/cmd_dispatch.rs` into grouped command routing modules by domain.
- [x] Split `src/tui_shell/modal.rs` into modal state transitions and rendering helpers.

Progress notes:
- Started `app.rs` decomposition by extracting core mode/context/time-display enums and impls into:
  - `src/tui_shell/app/types.rs`
- Continued `app.rs` decomposition by extracting modal and text-input action types into:
  - `src/tui_shell/app/modal_types.rs`
- Continued `app.rs` decomposition by extracting log and command metadata structs into:
  - `src/tui_shell/app/log_types.rs`
- Continued `app.rs` decomposition by extracting root-context color and release-summary helpers into:
  - `src/tui_shell/app/root_style.rs`
  - `src/tui_shell/app/release_summary.rs`
- Completed staged `app.rs` decomposition by extracting:
  - `src/tui_shell/app/state.rs` (app state declarations + default wiring)
  - `src/tui_shell/app/runtime.rs` (TTY/terminal bootstrap run path)
  leaving `src/tui_shell/app.rs` as a thin composition/re-export layer.
- Completed `cmd_dispatch.rs` decomposition into grouped modules:
  - `src/tui_shell/app/cmd_dispatch/root_dispatch.rs`
  - `src/tui_shell/app/cmd_dispatch/mode_dispatch.rs`
  with `src/tui_shell/app/cmd_dispatch/mod.rs` keeping suggestion/palette orchestration and top-level dispatch flow.
- Completed `modal.rs` decomposition into focused modules:
  - `src/tui_shell/modal/draw.rs` (rendering/layout/cursor)
  - `src/tui_shell/modal/keymap.rs` (key-to-action mapping and modal input edits)
  - `src/tui_shell/modal/text_input_validate.rs` (text-input validation/allow-empty rules)
  with `src/tui_shell/modal/mod.rs` as orchestration for applying modal actions into app side-effects.

### D) Server/CLI Entry Decomposition

- [x] Split `src/bin/converge-server.rs` into entry/runtime wiring modules while preserving route and state composition.
- [x] Reduce root-file wildcard imports where decomposition allows narrower imports.
- [x] Minimize cross-module visibility (`pub` -> `pub(crate)`/private where possible) after extraction.

Progress notes:
- Started `converge-server.rs` decomposition by extracting server state/domain type declarations into:
  - `src/bin/converge_server/types.rs`
  with `src/bin/converge-server.rs` now focused more on bootstrap/runtime wiring.
- Continued `converge-server.rs` decomposition by extracting runtime bootstrap and shutdown flow into:
  - `src/bin/converge_server/runtime.rs`
  leaving `src/bin/converge-server.rs` as module composition + top-level `main`.
- Reduced root entry wildcard imports by removing wildcard re-exports for runtime-only modules:
  - `handlers_system`
  - `routes`
- Tightened cross-module imports after extraction:
  - `runtime.rs` now imports identity functions directly from `identity_store` instead of relying on transitive re-exports.
  - `routes.rs` now imports `require_bearer` directly from `handlers_system`.
  - Server modules continue to use `pub(super)`/private visibility; no root-level `pub` exports are required.

### E) Regression and Verification

- [ ] Add focused tests for newly extracted boundaries where current coverage is indirect.
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment instability recurs).

## Exit Criteria

- Each primary Wave 4 target is reduced to a thin orchestration layer.
- Ownership and module boundaries are explicit in code and roadmap notes.
- Lint/tests pass after decomposition slices.
