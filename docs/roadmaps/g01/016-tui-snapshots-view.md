# Phase 016: TUI Snapshot Browser View

## Goal

Provide a dedicated, readable snapshot browser that feels like the earlier ratatui list/detail screens, while still keeping the command input as the primary way to act.

## Scope

In scope:
- A `snaps` mode/view that lists local snapshots with selection.
- A detail pane (or lower detail section) for the selected snap.
- Mode-local commands for common actions.

Out of scope:
- Remote browsing (Phase 017).

## Tasks

### A) Layout + navigation

- [x] List snaps (id prefix, timestamp, message) with keyboard selection.
- [x] Show details for selected snap (full id, created_at, root_manifest, stats, message).
- [x] Add a simple filter/search within snaps mode.

### B) Mode-local commands

- [x] `open <snap_id>`: select a snap by id/prefix.
- [x] `restore [--force]`: restore the selected snap.
- [x] `show`: print a short snap summary into the status area.
- [x] `back`: return to Root mode (alias for Esc).

## Exit Criteria

- `snaps` enters a snapshot browser.
- User can navigate, filter, and restore without returning to the shell scrollback model.
