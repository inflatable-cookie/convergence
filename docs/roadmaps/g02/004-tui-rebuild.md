# 004 TUI Rebuild

Status: complete
Owner: repo maintainers
Updated: 2026-07-24

## Context

The vertical slice (`g02.003`) is complete: client, CLI, server, and wire all
work end to end. Per architecture doc 15, the TUI starts once the CLI surface
is stable. This roadmap rebuilds the TUI against the captured UX spec
(`docs/rebuild/002-tui-ux-spec.md`): preserve the eight interaction
principles, fix the seven warts.

## Goals

- `converge-tui` as a thin front-end over the CLI command layer (argv
  contract; no bespoke semantics)
- single-screen console shell: five regions, command input + fuzzy palette,
  view stack, layered Esc, state-computed primary action
- async remote operations (UX spec wart 1) — the event loop never blocks
- agent trace as a first-class capability

## Non-Goals

- new verbs or server behavior (route through CLI/server roadmaps)
- workflow profiles and releases (later roadmap)

## Execution Plan

### Batch 4.1 - Command Layer and Shell Core

- [x] expose `converge-cli` as a library: `execute(argv) -> JSON envelope`
      (same code path as the binary)
- [x] ratatui shell: five-region layout, command input with history,
      fuzzy suggestions palette, view stack, layered Esc (with explicit
      quit confirm — wart fix), Tab Local/Remote toggle with labeled prompt
      (wart fix)
- [x] local root view (changes summary, latest snap) + history view over
      the command layer; Enter runs the state-computed primary action

### Batch 4.2 - Remote Views and Async Ops

- [x] remote root dashboard (bundle status, recommended next actions)
- [x] async command runner: remote commands run off-thread, progress in the
      Last strip, UI never freezes (wart fix)

### Batch 4.3 - Wizards and Resolution View

- [x] wizard framework with back-one-step and review step (wart fix);
      structured option prompts, unrecognized input errors (wart fix)
- [x] publish/login wizards; superposition resolution view with variant
      keys and live validation

### Batch 4.4 - Agent Trace

- [x] JSONL semantic trace: screen_view (selectable items, primary CTA),
      user_action, state_change, classified errors, deduped by signature

## Exit Criteria

- TUI drives the full local + remote verb surface through the CLI layer
- UX spec sections 1-6 implemented; all seven section-7 warts fixed
- `effigy validate` green

## Outcome

All four batches complete. UX spec sections 1-5 implemented and all seven
section-7 warts fixed (async remote, Alt-jump keys, wizard back/review,
structured options, quit confirm, named context, no log pane). Workflow
profiles (spec §4.6) were an explicit non-goal here and move to a later
roadmap with releases.

## Next Task

Ask operator intent for the next execution owner (`g02.005` candidates:
releases/retention/GC, external backends (Postgres/S3), real identity,
edge nodes, workflow profiles).
