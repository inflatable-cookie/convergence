# Convergence Product Guardrails

Convergence is an experimental version-control and collaboration system.

These guardrails define what must remain true as the repo moves from research
and foundational proving into the next execution sequence.

## Product Guardrails

- Keep the core object model coherent: `snap`, `publish`, `candidate`, `promote`,
  `release`, and `superposition` must stay stable and explicit across docs and
  implementation.
- Do not let research, architecture, and execution blur into one vague queue.
  Research can inform the product, but active implementation work must be named
  as an explicit owner.
- Preserve the large-organization workflow focus without inventing a separate
  small-team product mode by drift.
- Keep CLI and TUI semantics aligned to one underlying model instead of letting
  one surface become the real source of truth.
- Prefer explicit gate, identity, provenance, and authority rules over
  Git-shaped convenience assumptions.
- If there is no honest next execution owner after the research tranche, the
  repo should say it is paused in planning rather than inventing placeholder
  implementation work.

## Anti-Patterns

- reopening completed research as if it were active implementation
- creating new execution lanes without naming which part of the Convergence
  model they advance
- letting raw operator notes or ad hoc discussions replace roadmap authority
- widening a paused planning gate into freeform product invention

## Next Task

Use these guardrails to keep Convergence in a strict post-research planning
posture until the next real execution boundary is explicit.
