# 019 Auto-Capture

Status: complete
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

## Outcome

- `SnapRecord.trigger` ("explicit"/"automatic", metadata only);
  `create_snap_with`; CLI `snap` stays explicit
- `converge watch [--interval-ms] [--once]`: poll-based root-hash watcher;
  captures only when the tree is stable across ticks and differs from
  head (idempotent recapture makes quiet ticks free); runs thinning after
  each capture
- thinning: age tiers (keep < 1h, newest per hour < 1d, newest per day
  beyond); explicit snaps and head never thinned; injected `now` keeps
  policy testable
- thinning-vs-lineage ruled in doc 17 first (stop-condition path):
  re-parenting is impossible by construction (ids embed parent ids), so
  thinned ancestors are expected gaps; lineage walk degrades to timestamp
  order past a gap, which coincides with lineage order in thinned history
- tests: tier policy incl. explicit/head immunity, gap-tolerant walk,
  `watch --once` captures-on-change / silent-when-quiet through the
  binary; 61 workspace tests green

## Next Task

Execute the Batch 6.2 workspace-status card (`020-workspace-status.md`).
