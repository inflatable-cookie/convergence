# 030 Northstar Instruction And Rust Quality Audit

Status: in progress
Owner: repo maintainers
Updated: 2026-08-31

## Objective

Audit Convergence's always-loaded agent instructions and the full Rust
workspace under Northstar's explicit strict audit procedures. Repair only
recorded, authority-bounded findings, preserve product and public contracts,
and leave one reviewable evidence-backed PR.

## Governing Authority

- `AGENTS.md` and `CLAUDE.md`
- `docs/contracts/001-working-rules.md`
- `docs/contracts/rust-quality-profile.json`
- `docs/contracts/rust-quality-deviations.json`
- Northstar's agent-instruction review and Rust explicit audit modes

## Runway

- [`batch-cards/101-northstar-agents-and-rust-audit.md`](./batch-cards/101-northstar-agents-and-rust-audit.md) — ready

## Boundaries

- No release cut or release-pipeline mutation.
- No `.github/workflows/` changes.
- No product redesign, public API break, MSRV change, foreign error-policy
  choice, or unsafe repair without operator direction.
- Existing god-file and attention-marker reports are leads, not blanket repair
  authority.
- The AGENTS review may change instruction surfaces and its evidence only; it
  must preserve the Rust activation block and project-specific safety rules.

## Acceptance

- The repository-scope Rust audit is initialized, assessed, completed, and
  finalized through Northstar's recorder, with every unit and limitation
  accounted for.
- Only findings with repair authority are changed; report-only and
  operator-decision findings remain explicit and unmodified.
- `AGENTS.md` gives an unfamiliar agent a clear project-first reader journey;
  `CLAUDE.md` is the exact bridge unless a real Claude-only rule is proven.
- Repository-native validation passes, or a failure that changes the plan is
  returned to the orchestrator.
- The worker opens a PR for independent orchestrator review and does not merge.

## Review Oracle

| Invariant | Smallest counterexample | Expected stop or proof |
| --- | --- | --- |
| Audit scope is complete | One crate, target, public surface, unsafe boundary, or Rust file is absent from the recorder plan | Finalization or review stops until the omission is accounted for |
| Repairs stay within authority | A report-only unsafe/slop finding, MSRV raise, public break, or foreign error-policy choice is changed | Worker stops and returns the decision; diff contains no such mutation |
| Existing behavior survives | A quality rewrite changes CLI, wire, persistence, authz, or release semantics | Focused tests plus `effigy qa` prove preservation; otherwise stop |
| Instruction intent survives compression | A release/workflow boundary, strict continuation rule, Effigy contract, or Rust activation disappears | Section map and final diff show the boundary retained clearly |
| Claude bridge stays a bridge | Generic repo guidance remains duplicated in `CLAUDE.md` | `CLAUDE.md` is exactly `@AGENTS.md` unless the PR proves a Claude-only need |

## Next Task

Run card 101 through one isolated worker lane, then review its exact PR head.
