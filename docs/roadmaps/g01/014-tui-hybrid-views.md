# Phase 014: TUI Hybrid Views (Optional Panels)

Status: implemented as an intermediate step. Later phases will replace the split-panel approach with modal, view-driven screens while keeping the command input.

## Goal

Retain the command shell as the primary interaction surface while providing optional rich panels for high-volume remote browsing (inbox, bundles, superpositions).

Shell remains always available and is not replaced.

## Scope

In scope:
- Keep a single global command input.
- Add optional split views for:
  - Inbox (publications)
  - Bundles
  - Superpositions inspector
- Provide commands that open these views (`inbox`, `bundles`, `superpositions`).

Out of scope:
- Full-screen editor integration.
- Multi-tab panes.

## Tasks

### A) Shell + view composition

- [x] Make Shell the baseline layout.
- [x] Add a "panel" area that can be opened/closed.
- [x] Ensure command execution always writes to scrollback (even if a panel is open).

### B) Inbox panel

- [x] Remote-backed list with selection.
- [x] Show publication details (id, snap, publisher, created_at).
- [x] Add filter in panel (shell command remains global).

### C) Bundles panel

- [x] Remote-backed list with selection.
- [x] Show bundle details (id, promotable, reasons).
- [x] Add filter in panel (shell command remains global).
- [x] Actions: approve, promote, view superpositions (via panel shortcuts that prefill commands).

### D) Superpositions panel

- [x] Navigate conflicted paths.
- [x] Filter conflicted paths via `superpositions --filter`.
- [x] Show variant details + VariantKey JSON.
- [x] Integrate decision picking and validation status.

## Exit Criteria

- Users can either stay entirely in shell mode or open panels for browsing.
- Panels never prevent running commands.
