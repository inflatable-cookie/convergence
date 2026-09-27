# Convergence Product Guardrails

Convergence is an experimental version-control and collaboration system.

These guardrails define what must remain true as the product moves.

## Product Guardrails

- Keep the core object model coherent: `snap`, `publish`, `candidate`, `promote`,
  `release`, and `superposition` must stay stable and explicit across docs and
  implementation.
- Research can inform the product; active implementation work must be named
  as an explicit lane in the project's Queue plan.
- Preserve the large-organization workflow focus without inventing a separate
  small-team product mode by drift.
- Keep CLI and TUI semantics aligned to one underlying model instead of letting
  one surface become the real source of truth.
- Prefer explicit gate, identity, provenance, and authority rules over
  Git-shaped convenience assumptions.
- If there is no honest next execution owner, the plan should say so rather
  than inventing placeholder implementation work.

## Anti-Patterns

- reopening completed research as if it were active implementation
- creating new execution lanes without naming which part of the Convergence
  model they advance
- letting raw operator notes replace knowledge or plan authority
- widening a paused planning gate into freeform product invention
