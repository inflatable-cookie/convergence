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
- Candidates and promotions become the canonical integration language across teams.
- Release channels are policy-driven outputs, not the only moment work becomes meaningful.

## Product posture

- The architecture is designed for large-organization workflows; the same
  model serves solo and small teams through lighter deployments rather than
  a separate product mode.
- **Adoption beachhead: binary-heavy small teams** (DAW, game assets, VFX)
  where git is weakest and Perforce is the incumbent. Large-org gated
  convergence is the growth story, not the entry story.
- **Git interop is first-class.** Nobody migrates cold: Convergence must
  import existing git history, export mirror branches git tooling can
  consume, and coexist with `.git` in one tree.
- **Determinism is a product feature.** Candidate identities derive from their
  inputs; provenance replay can re-derive a candidate and verify its hash —
  the audit and compliance story for the org market.
- CLI and TUI should share one deterministic semantic contract.

## Constraints

- Terminology must stay stable: `snap`, `publish`, `candidate`, `promote`, `release`, `superposition`.
- Documentation should describe one coherent object model across local workspace, server, and operator flows.
- New milestones should derive from this vision and the architecture docs rather than inventing ad-hoc feature threads.

## Next Task

Use the closing `g02` roadmap sequence and `generation-index.md` strategic
horizons to sequence post-ship work without reintroducing Git-shaped
assumptions.
