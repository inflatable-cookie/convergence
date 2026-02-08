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

- [ ] Split `src/tui_shell/wizard/login_bootstrap_flow.rs` into focused stages (state transitions, validation, and effect execution).
- [ ] Split `src/tui_shell/views/root.rs` into view-model assembly, layout selection, and section rendering helpers.
- [ ] Keep top-level wizard/view modules as orchestration-only entry points.

Progress notes:
- Started `login_bootstrap_flow.rs` decomposition by extracting validation parsing helpers into `src/tui_shell/wizard/login_bootstrap_validate.rs` and wiring the flow to use them.

### C) TUI App Surface Decomposition

- [ ] Split `src/tui_shell/app.rs` into app state construction, mode switching, and event-loop dispatch helpers.
- [ ] Split `src/tui_shell/app/cmd_dispatch.rs` into grouped command routing modules by domain.
- [ ] Split `src/tui_shell/modal.rs` into modal state transitions and rendering helpers.

### D) Server/CLI Entry Decomposition

- [ ] Split `src/bin/converge-server.rs` into entry/runtime wiring modules while preserving route and state composition.
- [ ] Reduce root-file wildcard imports where decomposition allows narrower imports.
- [ ] Minimize cross-module visibility (`pub` -> `pub(crate)`/private where possible) after extraction.

### E) Regression and Verification

- [ ] Add focused tests for newly extracted boundaries where current coverage is indirect.
- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run` (or document fallback if environment instability recurs).

## Exit Criteria

- Each primary Wave 4 target is reduced to a thin orchestration layer.
- Ownership and module boundaries are explicit in code and roadmap notes.
- Lint/tests pass after decomposition slices.
