# 001 Post-Research Next-Boundary Gate

Status: complete (archived 2026-07-23; exit condition met — real owner opened
as `g02.002`, see `docs/specs/002-archive-and-rebuild-boundary.md`)
Updated: 2026-07-23
Roadmap: `g02.001`

## Context

Convergence's long `g01` sequence now mixes foundational implementation,
Northstar alignment, and a completed multi-phase research program. The repo no
longer has a clean active execution owner, but the docs still advertise `g01`
as active and suggest more roadmap work should continue there.

This spec creates a strict planning gate for the post-research posture. Its job
is to stop stale `g01` language from acting like a live queue and to hold the
repo until the next real execution owner is explicit.

## Governing Refs

- `docs/architecture/product-guardrails.md`
- `docs/contracts/001-working-rules.md`
- `docs/roadmaps/generation-index.md`
- `docs/roadmaps/README.md`
- `docs/roadmaps/g02/README.md`
- `docs/roadmaps/g02/001-post-research-execution-planning-gate.md`

## Lane Focus

The active strict lane is:

- close `g01` honestly as the foundational and research generation
- keep Convergence paused in planning until the next execution owner is named
- avoid fake implementation momentum after the research tranche

## Batch Model

- planning stays in this spec plus the roadmap
- execution proceeds only from a ready card
- if there is no honest next owner, the repo should remain paused explicitly

## Current State

- the strict pause install batch is complete
- there is currently no ready implementation or planning card
- the lane remains paused until the next honest owner is named

## Intent Checkpoint

If the next owner is still ambiguous after the planning gate, ask for intent
instead of guessing or reopening research by drift.

## Exit Condition

This strict lane is complete when Convergence either:

- opens a real post-research execution owner with a ready card
- or remains explicitly paused with coherent front doors and no ready card

## Next Task

Ask for intent or open a real next owner only when the next post-research
boundary is explicit; otherwise keep the repo in a coherent paused state.
