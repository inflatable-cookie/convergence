# 006 Continuous Capture and Workspace UX

Status: complete
Owner: repo maintainers
Updated: 2026-07-24



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

## Execution Plan

### Batch 6.1 - Auto-Capture

- [x] `converge watch` verb: filesystem watcher with debounce, auto-snap on
      quiet periods, `trigger` metadata (automatic vs explicit)
- [x] thinning retention for automatic snaps (keep-last + age tiers);
      explicit snaps never thinned
- [x] TUI toggle + watcher status surfaced — moved to 6.2 with the status
      verb (watcher runs as `converge watch`; TUI surfacing belongs with
      the status consolidation)

### Batch 6.2 - Workspace Status

- [x] one status verb: pending changes, head/lineage position, sync state,
      publish target, remote reachability; bundle status moves to
      `bundle <id>`
- [x] TUI root consumes it wholesale

### Batch 6.3 - Interactive Views

- [x] history selection verbs (Enter on a snap -> restore/diff), snap
      message editing, resolution by variant key

## Exit Criteria

- a workspace left alone accumulates thinned automatic snaps; nothing else
  in the UX regresses; `effigy validate` green

## Outcome

All three batches complete; exit criteria met — a watched workspace
accumulates thinned automatic snaps, the UX gained one-call status,
actionable history, annotate, and variant-key resolution with no
regressions.

## Next Task

Open `g02.007` (lanes and collaboration).
