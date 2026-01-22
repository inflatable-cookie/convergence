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

- [ ] Add a `converge` TUI entrypoint when invoked with no args.
- [ ] Choose a TUI framework (ratatui/crossterm) and wire terminal init/restore.
- [ ] Implement a basic app state loop (events, redraw, quit).

### B) Data model + API integration

- [ ] Define client-side view models for:
  - publications
  - bundles
  - promotion state
  - gate graph
- [ ] Add remote API client layer reusable by CLI and TUI.

### C) Screens

- [ ] Overview screen: show remote config + current scope + promotion state.
- [ ] Inbox screen: list publications by scope/gate; quick filter.
- [ ] Bundles screen: list bundles; show promotable + reasons.
- [ ] Bundle details: show inputs, root manifest id, created_at/by.

### D) Superposition inspection

- [ ] Detect superpositions in a bundle root manifest.
- [ ] Present a navigable list of conflicted paths.
- [ ] Show variant metadata (source publication id, kind, blob/manifest ids).

### E) Actions

- [ ] Trigger `bundle` creation for selected publications.
- [ ] Trigger `promote` for selected bundle.
- [ ] Trigger `approve` for selected bundle (if required approvals are enabled).

## Exit Criteria

- Running `converge` opens a stable TUI.
- A user can see publications, create a bundle, understand promotability, and promote/approve without leaving the TUI.
