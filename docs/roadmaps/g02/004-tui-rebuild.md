# 004 TUI Rebuild

Status: active
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

- [ ] remote root dashboard (bundle status, recommended next actions)
- [ ] async command runner: remote commands run off-thread, progress in the
      Last strip, UI never freezes (wart fix)

### Batch 4.3 - Wizards and Resolution View

- [ ] wizard framework with back-one-step and review step (wart fix);
      structured option prompts, unrecognized input errors (wart fix)
- [ ] publish/login wizards; superposition resolution view with variant
      keys and live validation

### Batch 4.4 - Agent Trace

- [ ] JSONL semantic trace: screen_view (selectable items, primary CTA),
      user_action, state_change, classified errors, deduped by signature

## Exit Criteria

- TUI drives the full local + remote verb surface through the CLI layer
- UX spec sections 1-6 implemented; all seven section-7 warts fixed
- `effigy validate` green

## Next Task

Execute the ready Batch 4.2 card
(`batch-cards/012-tui-remote-views-and-async.md`).
