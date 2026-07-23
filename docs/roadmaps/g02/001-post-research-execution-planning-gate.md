# 001 Post-Research Execution Planning Gate

Status: complete
Owner: repo maintainers
Updated: 2026-07-23

## Context

Convergence's `g01` sequence has already carried:

- foundational implementation work
- Northstar doctrine alignment
- a completed three-phase research program

The repo should no longer behave as if `g01` is still the live execution queue,
but it also should not invent a new implementation program until the next real
product boundary exists.

This roadmap is a paused planning gate. Its job is to hold the repo in an
explicit strict posture while it waits for a real next owner.

## Goals

- close `g01` as the active generation
- open one explicit post-research planning gate
- keep the repo ready to resume under strict control when a real next boundary
  appears
- avoid fake implementation churn in the absence of clear new work

## Non-Goals

- reopening completed research milestones for generic continuation
- claiming the next sequence before a real execution boundary exists
- hiding a paused state behind stale “active generation” language

## Execution Plan

### Batch 1.1 - Strict Pause Install

- [x] close `g01` as the active queue
- [x] open `g02.001` as the paused planning gate
- [x] align the front doors to the new strict posture

### Batch 1.2 - Next-Boundary Decision

- [x] review whether a real product or architecture boundary now justifies
      opening a new execution owner
- [x] either define that owner explicitly or keep Convergence paused without a
      ready implementation card

## Exit Criteria

- Convergence no longer advertises a stale active queue
- the paused post-research posture is explicit
- the next restart condition is clear

## Outcome

Batch 1.2 resolved on 2026-07-23: operator named the archive-and-rebuild
boundary. The new execution owner is `g02.002`
(`002-archive-and-rebuild-boundary.md`).

## Next Task

Continue under `g02.002`.
