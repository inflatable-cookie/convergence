# Phase 012: TUI Command Shell (Local-First)

Status: implemented as an intermediate step. The global command input remains, but later phases will shift away from an open-ended scrollback UI toward modal, view-driven screens.

## Goal

Make the TUI useful without any remote configuration or connectivity.

The TUI becomes a local-first command shell with a global input line, command history, and lightweight autocomplete.

## Scope

In scope:
- No remote HTTP calls on startup.
- A "Shell" UI that accepts commands (leading `/` optional).
- Local workflow commands: status, snap, snaps, show, restore.
- A mode indicator with `Tab` toggling Local/Remote only when the input is empty.

Out of scope:
- Remote commands (handled in Phase 013).
- Rich list/detail views (handled in Phase 014).
- Interactive per-command prompting contexts (handled in Phase 015).

## Tasks

### A) App lifecycle + modes

- [x] Refactor `src/tui.rs` startup to only discover workspace + config (no HTTP).
- [x] Add `Mode::{Local, Remote}` with a visible indicator.
- [x] Bind `Tab` to toggle mode only when input is empty.
- [x] Bind `q`/`esc` to quit (with `esc` first clearing input/palette).

### B) Shell UI

- [x] Add a scrollback model: timestamp + kind (command/output/error).
- [x] Add a single-line input editor:
  - cursor left/right
  - backspace/delete
  - ctrl-u clear line
  - history up/down
- [x] Add a suggestion/palette box below input:
  - fuzzy match command names
  - `Tab` cycles suggestions when input non-empty
  - `Enter` runs selected / best match

### C) Command parsing

- [x] Parse input as a command line; accept optional leading `/`.
- [x] Tokenize with quotes (e.g. `snap -m "msg"`).
- [x] Provide `help` with short usage lines.

### D) Local commands

- [x] `status`: workspace root, remote configured yes/no, snap count, latest snap id/time.
- [x] `init [--force]`: initialize `.converge` (for starting outside a workspace).
- [x] `snap [-m "..."]`: create a snap.
- [x] `snaps [--limit N]`: list snaps.
- [x] `show <snap_id>`: show snap details.
- [x] `restore <snap_id> [--force]`: restore snap.
- [x] `clear`: clear scrollback.
- [x] `quit`: exit.

## Exit Criteria

- Running `converge` (no args) inside a workspace provides a usable local-first shell.
- No remote configuration is required to use the TUI.
- `Tab` toggles Local/Remote only when input is empty; otherwise it behaves as completion.
