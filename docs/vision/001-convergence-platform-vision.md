# 001 Convergence Platform Vision

Status: active
Owner: Better Than Clay
Created: 2026-03-06

## Goal

Build Convergence into a version-control and collaboration system that captures real workspace state continuously, converges it through explicit gate policy, and makes intermediate outputs consumable without pretending every step is release-ready.

## Why this matters now

Git remains strong at source history, but it treats many modern workflows as awkward edge cases: large binary churn, unresolved integration state, gated organizational convergence, and operator-visible provenance across promotion steps. Convergence exists to make those constraints first-class instead of bolted-on.

## Long-horizon outcomes

- Local capture is cheap, deterministic, and safe even when work is incomplete.
- Server authority handles identity, permissions, gate graphs, scopes, and provenance cleanly.
- Superpositions are preserved as data and resolved deliberately instead of being collapsed into accidental merge behavior.
- Bundles and promotions become the canonical integration language across teams.
- Release channels are policy-driven outputs, not the only moment work becomes meaningful.

## Product posture

- Large-organization workflows are the primary design target.
- Solo and small-team use should reuse the same model through lighter deployments rather than a separate product mode.
- CLI and TUI should share one deterministic semantic contract.

## Constraints

- Terminology must stay stable: `snap`, `publish`, `bundle`, `promote`, `release`, `superposition`.
- Documentation should describe one coherent object model across local workspace, server, and operator flows.
- New milestones should derive from this vision and the architecture docs rather than inventing ad-hoc feature threads.

## Next Task

Use the architecture set and `g01` roadmap sequence to keep turning the object model, gate policy, remote workflow, and UX work into executable milestones without reintroducing Git-shaped assumptions.
