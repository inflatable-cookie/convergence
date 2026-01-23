# Phase 015: TUI Modal Modes (View-Driven)

## Goal

Keep the command input structure, but make the TUI view-driven and modal.

At the root, commands either:
- run as one-offs (e.g. `ping`, `help`) and show a short result, or
- switch into a mode (e.g. `snaps`, `inbox`, `bundles`, `superpositions`) that presents a dedicated ratatui view and a new set of mode-local commands.

`/` remains an optional prefix for commands. In modal mode, `/` can be used as an escape hatch to force root/global command resolution.

## Scope

In scope:
- A mode stack (root -> mode -> submode) with consistent back/escape semantics.
- Per-mode command sets + suggestions (no global “everything everywhere” palette).
- Replace the open-ended scrollback as the primary UI with:
  - the active view, and
  - a small status/notification area for the last command result/error.

Out of scope:
- Rewriting core converge workflows.
- A full TUI redesign for every screen in one pass (handled in later phases).

## Tasks

### A) Mode + view framework

- [x] Introduce a `UiMode` (or `ViewId`) enum and a mode stack.
- [x] Define a small `View` interface (render + input handling + optional command handlers).
- [x] Add root navigation commands that push/pop modes:
  - `snaps` -> Snapshot Browser
  - `inbox` -> Inbox Browser
  - `bundles` -> Bundles Browser
  - `superpositions` -> Superpositions Browser
- [x] Add universal navigation:
  - `Esc`: if input non-empty, clear input; else pop mode; at root, quit-confirm or quit.
  - `q`: quit (consistent across modes).

### B) Command scoping + suggestions

- [x] Command resolution order:
  - mode-local commands first
  - then global commands (`help`, `quit`, `ping`)
- [x] If the input starts with `/`, force root/global command resolution (even while inside a mode).
- [x] Suggestions show only commands relevant to the active mode.
- [x] `help` shows the active mode’s commands by default (and a way to ask for root/global help).

### C) Output model (no scrollback-first UI)

- [x] Replace the current scrollback UI with a small “last result” area:
  - last command (optional)
  - last output lines (bounded)
  - last error (bounded)
- [x] Keep an internal log buffer for debugging, but do not make it the primary screen.

## Exit Criteria

- Running `converge` starts in Root mode with a clean, view-driven layout.
- Typing `snaps` switches to a Snapshot Browser view; the command list/suggestions update.
- `Esc` returns to Root mode from any mode.
- `ping` works as a one-off without switching mode.
