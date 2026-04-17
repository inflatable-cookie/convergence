# 001 Working Rules

Status: active
Updated: 2026-04-09

This contract defines how Convergence executes active work under the strict
Northstar posture.

## Canonical Surfaces

Execution should anchor on these surfaces in order:

1. `docs/roadmaps/generation-index.md`
2. `docs/roadmaps/README.md`
3. the active generation README
4. the active roadmap milestone
5. `docs/specs/README.md`
6. the active spec and current ready batch card
7. `docs/logs/README.md`

Historical logs remain evidence, but they must not be the only live queue
authority.

## Ready-State Rule

Implementation work should only proceed when a bounded ready batch card exists.

If there is no ready card, the lane is in planning. Do not infer execution from
older research closures or stale `Next Task` text.

## Continue Rule

In the active strict lane, bare `continue` should resolve through the previous
closeout's `Next Task`.

That `Next Task` should normally point at the current ready card. If it does
not, refresh the active surfaces before more work continues.

## Closeout Rule

When a batch closes:

1. update the batch card
2. update the governing roadmap/spec if status or next-step state changed
3. refresh any front-door or currentness surface that still advertises the
   active lane or ready card
4. write one evidence log with validation actually run
5. leave one explicit `Next Task` in the highest-authority active surface

A completed card must never remain advertised as the current ready card.

## Intent Checkpoint Rule

When planning is needed and the next direction is materially ambiguous, stop
and ask for intent instead of guessing.

For Convergence that usually means naming whether the next move is:

- object-model implementation
- gate or authority workflow execution
- operator/bootstrap work
- or continued planning because no honest next owner exists yet

## Batch Scope Rule

Keep work bounded to one honest owner at a time. Do not mix research expansion,
platform invention, and UX polish into one vague continuation lane.

## Generation Rollover Rule

Treat roadmap generations as substantial sequencing eras, not tiny buckets. In a long-running repo, expect roughly 20 to 40 roadmap files in one generation before rollover is even worth discussing.

Treat rollover as full closeout:

- every roadmap in the old generation must be explicitly closed, paused, superseded, or moved to backlog
- the roadmap front doors must reflect that closed state before the next generation opens
- stale specs and batch cards from the closing generation must be archived or removed from `docs/specs/`

If those closeout conditions are not satisfied, repair the current generation instead of opening a new one.

## Next Task

Use these rules to hold Convergence in an explicit strict planning gate until a
real post-research execution owner exists.
