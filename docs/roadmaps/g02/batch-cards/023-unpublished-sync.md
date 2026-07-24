# 023 Unpublished Sync

Status: complete
Updated: 2026-07-24
Roadmap: `g02.007`
Spec: `docs/specs/007-lanes-and-collaboration.md`

## Objective

Share WIP without the gate: push/pull snap lineage to lane heads.

## In Scope

- lane heads carry lineage: server stores per-lane head snap id +
  uploaded snap records (snap records become server objects; extend the
  object/metadata surface deliberately)
- `converge sync push [--lane]`: upload head snap lineage (snaps +
  reachable objects, Merkle-pruned) and set the lane head; owner/member
  only
- `converge sync pull --lane <id>`: fetch a lane head's lineage into the
  local store (no workspace mutation; restore stays explicit); visibility
  enforced — private lanes readable by owner/members, repo lanes by repo
  readers
- lane head updates are fast-forward-checked against the uploaded
  lineage (non-FF requires `--force`, recorded)
- e2e: alice pushes WIP to a shared lane, bob pulls it, restores the
  snap, lineage (parents) intact; visibility denial test

## Out Of Scope

- inbox (7.3), provenance tightening (7.4)

## Acceptance Criteria

- two clients share unpublished lineage through a lane with visibility
  enforced; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- lineage transfer semantics get murky — doc 17 first

## Outcome

- snap records are server objects with verify-on-write identity checks
  (`snap_records` table, PUT/GET routes); lane heads persisted per repo
- `set_lane_head`: writable-lane resolution shared with publish (personal
  auto-provision), fast-forward ancestry check over uploaded records,
  `force` override
- client `push_lineage` (chain upload deepest-first, Merkle-pruned trees)
  and `pull_lane` (lineage walk into the local store, tolerant of thinned
  gaps; restore stays explicit); CLI `sync push/pull`
- lane-head reads enforce visibility: owner/members always, repo-visible
  lanes for repo readers; private personal lanes deny others
- e2e: alice pushes 2-snap lineage, bob pulls with parents intact and
  restores; non-FF push refused; private-head read denied.
  Route-registration bug caught by the tests (fmt reflow ate a patch
  anchor — handlers existed, routes did not)
- 71 workspace tests green

## Next Task

Execute the Batch 7.3 inbox card (`024-inbox.md`).
