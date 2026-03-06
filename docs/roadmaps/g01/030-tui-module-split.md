# Phase 030: TUI Module Split

## Goal

Replace the single "god file" TUI implementation with a small, discoverable module tree.

Keep behavior identical (or nearly identical), keep tests passing, and make it easy to find:
- command definitions + parsing
- the app/event loop/state machine
- views (History, Inbox, Bundles, etc.)
- modals + wizards
- diff/status helpers

## Scope

This phase is limited to internal Rust module structure.
No product/UX changes except tiny wiring fixes required by the refactor.

## Tasks

### A) Establish Module Layout

- [x] Create `src/tui_shell/` submodules.
- [x] Convert `src/tui_shell.rs` into a thin entry point (`src/tui_shell/mod.rs` + `src/tui_shell/app.rs`).
- [x] Add a short `src/tui_shell/README.md` documenting "where things live".

### B) Extract Pure Types + Helpers

- [x] Move `Input` to `src/tui_shell/input.rs`.
- [x] Move `CommandDef` + `*_command_defs()` to `src/tui_shell/commands.rs`.
- [x] Move suggestion scoring (`score_match`) and tests to `src/tui_shell/suggest.rs`.

### C) Extract UI Components

- [x] Move `View`/`RenderCtx` + chrome helpers to `src/tui_shell/view.rs`.
- [x] Split remaining views into `src/tui_shell/views/*.rs` (root).
- [x] Move History (Snaps) view to `src/tui_shell/views/snaps.rs`.
- [x] Move Inbox/Bundles/Lanes/Releases views into `src/tui_shell/views/`.
- [x] Move Settings and Superpositions views into `src/tui_shell/views/`.
- [x] Move modal rendering + handling to `src/tui_shell/modal.rs`.
- [x] Move wizards to `src/tui_shell/wizard.rs`.

### D) Extract Status/Diff Logic

- [x] Move `local_status_lines`/`remote_status_lines` + diff helpers into `src/tui_shell/status.rs`.

## Exit Criteria

- `src/tui_shell/mod.rs` is <= ~300 lines and mostly `mod ...;` + shared imports + top-level entry.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
