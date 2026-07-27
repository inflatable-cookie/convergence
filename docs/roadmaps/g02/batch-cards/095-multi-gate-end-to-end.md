# 095 Multi-Gate End To End

Status: complete
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

## Outcome

Findings in
`docs/logs/2026-07/27-220000-batch-26-4-multi-gate-findings.md`. The
staged flow now works end to end on the real repo: publish into intake,
promote to review, approve, promote to release, release from the gate
that may.

Four defects, none of which any test could have seen, because no repo
had ever had a second gate. Three were one assumption wearing three
hats — that a bundle is only ever at the gate that produced it:

- promotion checked the target's upstreams against the producing gate,
  so any gate whose upstream was not an entry gate was **unreachable**
- `required_approvals` was read off the producing gate, so a review
  stage's approval count was configuration that did nothing
- `release` read `may_release` off the producing gate, so a bundle
  promoted into a release gate could not be released from it

The fourth came from batch 22.4's own prefix resolution: every verb that
*records* a bundle id wrote back whatever the caller typed, so approvals,
promotions and releases held twelve-character ids referencing no bundle.
GC protects released bundles by comparing ids, and a truncated id never
matches — that one would have been silent.

Both regression tests were checked by stashing the fix and re-running,
which is now the habit: the retention test in 22.4 passed either way on
its first draft.

The measurement findings 10 and 34 asked for: before any promotion the
window floor is 0 and no publication is eligible for collection whatever
the policy says. After the first real promotion, 14 publications became
droppable and 33 objects (21.5 KB) sweepable, reachable falling 134 to
101. The causal story in those findings is confirmed, and gate
administration is what unblocks it.

## Next Task

Roadmap `g02.026` closes. `22.5` (release) becomes the operator's call
again.
