# TUI Shell Layout

This folder contains the interactive TUI implementation.

Where to look:

- `src/tui_shell.rs`: entry point + `App` + event loop wiring (still the largest file during the split).
- `src/tui_shell/commands.rs`: command palette definitions (what `/` can show).
- `src/tui_shell/input.rs`: input editing + history.
- `src/tui_shell/suggest.rs`: palette matching + sorting.

Planned next (Phase 030):

- `src/tui_shell/view.rs`: view trait + shared chrome.
- `src/tui_shell/views/`: one file per view (History, Inbox, Bundles, ...).
- `src/tui_shell/modal.rs`: modal rendering + key handling.
- `src/tui_shell/wizard.rs`: multi-step wizards.
- `src/tui_shell/status.rs`: local/remote status + diff helpers.
