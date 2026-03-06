# Phase 004: TUI (Inbox, Inspection, Resolution)

## Goal

Add an interactive terminal UI (`converge` with no args) that supports day-to-day convergence work:
- see what has been published to your lane/scope
- inspect publications, bundles, promotion state
- inspect superpositions (conflicts) and understand why bundles are not promotable

This phase should not change semantics; it should be a UI on top of existing deterministic CLI/API behavior.

## Scope

In scope:
- A Rust TUI that talks to the same client library/commands as the deterministic CLI.
- Primary screens:
  - Overview (repo, scope, gate graph, promotion state)
  - Inbox (publications relevant to your current gate responsibilities)
  - Bundles (list, details, promotability reasons)
  - Superpositions (browse paths/variants; show provenance)
- Keyboard-first navigation and quick filtering.

Out of scope:
- Automated resolution/merge strategies.
- Advanced diff viewers.
- Full policy editor.

## Tasks

### A) TUI skeleton

- [x] Add a `converge` TUI entrypoint when invoked with no args.
- [x] Choose a TUI framework (ratatui/crossterm) and wire terminal init/restore.
- [x] Implement a basic app state loop (events, redraw, quit).

### B) Data model + API integration

- [x] Define client-side view models for:
  - publications
  - bundles
  - promotion state
  - gate graph
- [x] Add remote API client layer reusable by CLI and TUI.

### C) Screens

- [x] Overview screen: show remote config + current scope + promotion state.
- [x] Inbox screen: list publications by scope/gate; quick filter.
- [x] Bundles screen: list bundles; show promotable + reasons.
- [x] Bundle details: show inputs, root manifest id, created_at/by.

### D) Superposition inspection

- [x] Detect superpositions in a bundle root manifest.
- [x] Present a navigable list of conflicted paths.
- [x] Show variant metadata (source publication id, kind, blob/manifest ids).

### E) Actions

- [x] Trigger `bundle` creation for selected publications.
- [x] Trigger `promote` for selected bundle.
- [x] Trigger `approve` for selected bundle (if required approvals are enabled).

## Exit Criteria

- Running `converge` opens a stable TUI.
- A user can see publications, create a bundle, understand promotability, and promote/approve without leaving the TUI.
