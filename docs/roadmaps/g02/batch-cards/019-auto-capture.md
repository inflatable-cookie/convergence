# 019 Auto-Capture

Status: ready
Updated: 2026-07-24
Roadmap: `g02.006`
Spec: `docs/specs/006-continuous-capture-and-workspace-ux.md`

## Objective

Snaps become automatic: watch the workspace, capture on quiet periods,
thin the automatic history.

## In Scope

- `SnapRecord.trigger` metadata (`explicit` | `automatic`); CLI `snap`
  marks explicit
- `converge watch` verb: filesystem watcher (notify crate or poll-based),
  debounce window (default ~2s quiet), captures via the normal lineage
  path (idempotent recapture makes no-change ticks free)
- thinning: automatic snaps compact by age tier (keep all < 1h, hourly
  < 1d, daily beyond); explicit snaps and lineage anchors (parents of kept
  snaps re-parented honestly or retained) never silently vanish — define
  the re-parenting rule in the card outcome if non-obvious
- `--once` flag for tests; watcher loop testable without real timing where
  possible

## Out Of Scope

- TUI watcher toggle (6.2 surfaces status), status verb (6.2)

## Acceptance Criteria

- editing files under `converge watch` produces automatic snaps with
  correct lineage; quiet workspace produces none
- thinning removes only automatic snaps per policy
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- thinning vs lineage interaction turns out semantically murky — route
  through doc 17 before implementing

## Next Task

On completion, open the Batch 6.2 workspace-status card.
