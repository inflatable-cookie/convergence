# Phase 029: TUI Prompted Commands

## Goal

Make every TUI command feel "wizard-like" and low-friction:
- Commands that need parameters should prompt for them (instead of requiring the user to remember syntax).
- Commands that can safely default should still work with zero args (Enter should do the obvious thing).
- Power users can still paste full command lines, but nothing *requires* knowing `--flags`.

This phase is scoped to the TUI command shell (`src/tui_shell.rs`). The standalone CLI may keep flag-based UX.

## Tasks

### A) Prompt Infrastructure

- [x] Add a small, reusable prompt/wizard mechanism (multi-step text prompts).
- [x] Ensure Esc cancels the current wizard cleanly.

### B) Convert Key Commands

- [x] `login`: `login` starts a guided prompt (url, token, repo, scope, gate).
- [x] `publish`: `publish` opens a prompt; Enter publishes with defaults; `publish edit` customizes.
- [x] `fetch`: `fetch` in root can prompt for what to fetch (snap/bundle/release/lane).
- [x] `init`: accept `init [force]`.
- [x] `save`: accept `save [message...]`.
- [x] `history`: accept `history [N]`.
- [x] `restore`: accept `restore <snap> [force]`.
- [x] `purge`: accept `purge [dry]`.
- [x] `sync`: add a guided prompt when invoked with no args.

### C) Cover Remaining Commands

- [x] Inbox/bundles filters: prompt for filter/limit/scope/gate when requested.
- [x] Bundles actions: prompt for `to gate` / `channel` / `notes` when missing.
- [x] Admin: `member` / `lane-member` guided prompts.

## Exit Criteria

- No command in the TUI requires remembering `--flags`.
- Typing just the command name is sufficient to get guided prompts when parameters are needed.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
