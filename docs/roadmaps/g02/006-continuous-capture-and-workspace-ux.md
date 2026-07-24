# 006 Continuous Capture and Workspace UX

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

Opens after `g02.005`.

## Context

"Capture is continuous and invisible" is the core product bet and after two
generations snaps are still manual. This roadmap makes capture automatic and
finishes the workspace UX the rebuild left read-only.

## Planned Batches

- **6.1 Auto-capture**: file watcher, debounced auto-snap on quiet periods,
  capture `trigger` metadata (automatic vs explicit), retention thinning
  policy for automatic snaps (keep-last + age-based); off by default until
  hardened, `converge watch` / TUI toggle
- **6.2 Workspace status**: one `status`-equivalent verb (pending changes,
  head/lineage position, sync state, publish target) replacing the current
  overloaded `status`; TUI root consumes it wholesale
- **6.3 Interactive views**: selected-item verb layer in history (Enter →
  restore/diff against selection), snap message editing after capture,
  resolution view keyed by variant keys not indexes

## Exit Criteria

- a workspace left alone accumulates thinned automatic snaps; nothing else
  in the UX regresses; `effigy validate` green

## Next Task

Compile into batches with a ready card when `g02.005` closes.
