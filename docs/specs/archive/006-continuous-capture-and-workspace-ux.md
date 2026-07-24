# 006 Continuous Capture and Workspace UX

Status: complete
Updated: 2026-07-24
Roadmap: `g02.006`

## Governing Refs

- `docs/roadmaps/g02/006-continuous-capture-and-workspace-ux.md`
- `docs/architecture/17-lineage-and-merge-semantics.md` (lineage rules the
  watcher must respect: idempotent recapture, head parenting)
- `docs/rebuild/002-tui-ux-spec.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

The core product bet lands here: capture becomes continuous and invisible.
Auto-capture must never block the user's editor or corrupt lineage; when in
doubt the watcher skips a capture rather than guessing.

## Current State

- Batch 6.1 (auto-capture) complete.
- Batch 6.2 (workspace status) complete.
- All three batches complete; roadmap `g02.006` closed.

## Exit Condition

Roadmap `g02.006` exit criteria met.

## Next Task

Superseded by `docs/specs/007-lanes-and-collaboration.md`.
