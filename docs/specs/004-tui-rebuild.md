# 004 TUI Rebuild

Status: active
Updated: 2026-07-24
Roadmap: `g02.004`

## Governing Refs

- `docs/rebuild/002-tui-ux-spec.md` (UX authority)
- `docs/architecture/15-client-and-tui-architecture.md`
- `docs/roadmaps/g02/004-tui-rebuild.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

TUI is a thin front-end: every action assembles argv and calls the CLI
command layer. Any need for new semantics stops the batch and routes through
the CLI surface first.

## Current State

- Batch 4.1 (command layer and shell core) complete.
- Batch 4.2 has a ready card:
  `docs/roadmaps/g02/batch-cards/012-tui-remote-views-and-async.md`

## Exit Condition

Roadmap `g02.004` exit criteria met.

## Next Task

Execute the ready Batch 4.2 card.
