# 023 Unpublished Sync

Status: ready
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

## Next Task

On completion, open the Batch 7.3 inbox card.
