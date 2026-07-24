# 047 Promotion Guards

Status: complete
Updated: 2026-07-24
Roadmap: `g02.013`

## Objective

Audit H1 closed: promote refuses window regression. Today a stale but
promotable bundle (built before the current W was promoted) passes all
policy checks and rewinds the partition floor, re-opening consumed
publications — silent corruption of promoted history.

## In Scope

- monotonicity guard in promote: `bundle.window.1 > floor` required to
  advance; refusal names the stale window and current floor
- base-must-match-W guard: `bundle.base_bundle_id` must equal the
  partition's current `base_bundle_id` — a bundle folded onto an older
  W cannot become the new W
- fan-out re-promotion stays legal: when the bundle already is the
  current W (base == bundle_id, floor == window.1), promote to another
  downstream gate records the promotion without touching partition
  state
- doc 14 §3 amended first: partition serialization is optimistic
  guarded batches (assert + rollback + bounded retry) over a
  transactional connection, not a row-lock single writer; promotion
  monotonicity stated as an invariant
- tests: stale-bundle promote refused with a clear error after a newer
  bundle promoted; wrong-base promote refused; re-promote of the
  current W to a second downstream gate succeeds without state change

## Out Of Scope

- merge decision table (13.3); resolution/identity hygiene (13.4)

## Acceptance Criteria

- stale promote and wrong-base promote both refused with named
  windows/bases; fan-out re-promotion works; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- doc 14 §3 amended first: serialization is optimistic guarded batches
  (assert + rollback + bounded retry) over transactional connections,
  not a row-lock single writer; promotion monotonicity stated as an
  invariant
- promote guards (checked against the partition read that the batch
  asserts, so they cannot be raced): `window.1 > floor` else "stale
  bundle … republish against the current W"; base must equal the
  partition's current W else "would fork promoted history"
- fan-out: when the bundle already is the current W, promote to another
  downstream gate records the promotion only — partition state
  untouched
- tests: stale promote refused after a newer bundle promoted (floor
  never rewinds), wrong-base promote refused, current-W re-promotes to
  a second downstream gate with state unchanged; 119 tests green

## Next Task

Batch card 13.3 (merge decision-table fix).
