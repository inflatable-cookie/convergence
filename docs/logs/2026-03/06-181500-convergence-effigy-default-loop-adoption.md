# Convergence Effigy Default Loop Adoption

Date: 2026-03-06
Batch: `repo-surface normalization`

## Summary

- Promoted Effigy to the explicit default command surface for Convergence contributor work.
- Added root `health`, `validate`, and `qa` tasks so the repo has a standard Effigy-first development loop.
- Updated agent-facing docs so `effigy test` is the known default for tests, with `cargo nextest` used underneath when available.

## Changes

- Expanded [`effigy.toml`](~/Dev/projects/convergence/effigy.toml) to include `check`, `fmt:check`, `clippy`, `health`, `validate`, `qa`, and clearer runtime aliases alongside the existing run and trace tasks.
- Updated [`README.md`](~/Dev/projects/convergence/README.md), [`AGENTS.md`](~/Dev/projects/convergence/AGENTS.md), [`docs/README.md`](~/Dev/projects/convergence/docs/README.md), and [`docs/processes/260-agents-operating-guardrails.md`](~/Dev/projects/convergence/docs/processes/260-agents-operating-guardrails.md) to teach the Effigy-first loop.

## Validation Performed

- `effigy tasks`
- `effigy health`
- `effigy validate`
- `git diff --check`

## Evidence

- The repo now exposes a standard Effigy-first contract instead of relying on README-only Cargo commands.
- Test guidance now points at `effigy test`, making the nextest-preferred behavior discoverable to agents and contributors.

## Risks

- The local server bootstrap and login walkthroughs are still documented as raw command sequences rather than first-class Effigy scenario tasks.
- `effigy test` still depends on the existing Node wrapper, so the default behavior is clearer even though the underlying implementation remains the same.
- The repo still does not expose first-class Effigy scenario tasks for server bootstrap, login, or onboarding flows, so those operator loops remain docs-driven rather than task-driven.

## Next Task

- Decide whether Convergence should add first-class Effigy smoke tasks for local server bootstrap, login, and remote onboarding so the operator loop becomes Effigy-first as well.
