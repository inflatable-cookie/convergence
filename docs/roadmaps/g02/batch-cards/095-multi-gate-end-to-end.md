# 095 Multi-Gate End To End

Status: ready
Updated: 2026-07-27
Roadmap: `g02.026`

## Objective

Drive a real multi-gate repo the way batch 22.4 drove everything else,
and find what only use finds.

## Scope of the actual problem

This is the batch the roadmap exists for. Everything before it makes
`promote` reachable; this one asks whether the multi-gate flow actually
works, on real history, in somebody's hands.

It matters more than a normal test batch for a specific reason: the gate
machinery has never run. `promote` has engine tests and fixture
coverage, but no repo has ever had a second gate, so window advance,
`required_approvals` and `may_release` have only ever been exercised by
tests that built their own graph. 22.4's whole lesson was that the
defects live in the gap between what tests construct and what people do.

There is also a specific question to answer rather than assume. Findings
10 and 34 both trace to a window that never advances: a single-gate repo
can never GC published objects, and a wedged partition never drains. If
promotion advances the window, both should change. That is a claim, and
it should be measured on a real repo rather than reasoned about.

## In Scope

- a real repo given a staged graph — intake, review, release — after
  creation
- the full flow driven by hand: publish into intake, approve, promote to
  review, promote again, release from the gate that may release
- `required_approvals` enforced: a promotion refused for want of an
  approval, then allowed
- `may_release` respected: releasing from a gate that may not, refused
- the adversarial half: a graph change racing a publish, removing a gate
  with live bundles, re-parenting mid-flow
- **measured**: whether a window that advances lets GC reclaim objects it
  could not before (findings 10 and 34)
- findings recorded in `docs/logs/` in 22.4's shape, cheap ones fixed,
  expensive ones carded

## Out Of Scope

- releasing. `22.5` is still gated on this roadmap completing

## Acceptance Criteria

- the multi-gate flow works end to end, driven by hand, and the guides
  describe it
- promotion advances the window, and the effect on GC is measured and
  written down
- findings recorded; an explicit statement of whether the multi-gate flow
  is ready to put in front of anyone

## Validation

- `effigy validate`
- `effigy qa:docs`

## Next Task

Roadmap `g02.026` closes. `22.5` (release) becomes the operator's call
again.
