# 101 Northstar AGENTS And Rust Audit

Status: complete
Updated: 2026-08-31
Roadmap: `g02.030`

## Objective

Complete one repository-wide Northstar Rust audit-and-repair pass and one
target-aware AGENTS/CLAUDE optimization pass, then return a reviewable PR with
the recorder evidence and project closeout kept honest.

## Ordered Work

1. Run Northstar's explicit Rust audit at **repository** scope. Bootstrap and
   verify the pinned audit tools, resolve all six Cargo manifests and the
   repository MSRV, plan disjoint assessed units, initialize the recorder, and
   complete correctness, architecture, and human-quality assessments before
   editing each unit.
2. Record every finding and disposition. Repair only `review_required`
   findings with recorder-authorized plans. Keep unsafe and exact-forwarder
   findings report-only, and stop on any operator-decision item.
3. Finalize the Rust audit and use its generated report as evidence. Do not add
   generated audit records from Git metadata to the repository.
4. Run the target-local Northstar agent-instruction audit. Build a section
   intent/disposition map, then optimize only `AGENTS.md`, `CLAUDE.md`, and
   directly required evidence/docs. Preserve project voice and every safety,
   authority, continuation, Effigy, Rust-profile, workflow, and release
   boundary. Generic guidance already owned by `AGENTS.md` does not belong in
   `CLAUDE.md`.
5. Update this card, `g02.030`, affected front doors, and one dated log with the
   actual findings, repairs, limitations, measurements, and validation. Open a
   PR against `main`; do not merge.

## Scope

- Repository-scope Rust discovery across the root workspace and all five
  crates under `crates/`, including packages, targets, features, public API,
  unsafe/FFI, async/concurrency, persistence, wire, and test boundaries.
- Audit-authorized Rust, Cargo, focused test, and directly governed
  documentation repairs.
- `AGENTS.md`, `CLAUDE.md`, and the closeout evidence/currentness surfaces
  named above.

## Out Of Scope

- Release execution, workflow edits, product feature work, architecture
  replacement, broad god-file splitting, and unrelated marker cleanup.
- Public API or wire-format breaks, MSRV/toolchain changes, foreign error
  signaling policy, or unsafe-boundary repair without operator direction.
- Automatic deletion of exact forwarding wrappers.

## Acceptance

- Northstar's recorder finalizes the repository audit with all units,
  normative rule verdicts, forwarder candidates, limitations, evidence, and
  changed-file attribution complete.
- Every applied repair is the smallest change for a recorded authorized
  finding and has immutable passing evidence.
- Report-only, retained, excluded, and operator-decision surfaces remain
  byte-for-byte unchanged unless separately authorized by this card.
- The AGENTS section map covers the whole applicable instruction chain and the
  final reader journey explains Convergence, preservation intent, sharp edges,
  mechanics, and completion without generic noise.
- `CLAUDE.md` contains only `@AGENTS.md` unless a concrete Claude-specific rule
  is documented in the PR.
- `effigy qa` and the target-local agent-instruction check pass. Pre-existing
  doctor findings may remain only when identified as unchanged limitations.

## Stop Conditions

- The audit recorder cannot establish complete repository scope or valid
  strict policy.
- A finding requires an operator decision, public behavior change, new
  architecture, MSRV change, foreign error contract, or unsafe repair.
- Validation exposes a product defect or scope expansion that this card does
  not settle.
- The worktree is dirty at launch, the committed handoff cannot be verified,
  or any unrelated user work would be disturbed.

## Review Oracle

Use the invariant table in `g02.030`. During falsification, reconcile the
recorder result, diff, this card, roadmap, log, and front doors. The PR must
state every retained limitation; absence of a repair is not evidence of a
clean audit.

## Outcome

Both passes ran. The Rust audit finalized as `convergence-20260831-rust-audit`
with status `degraded`: six units, 26 evidence records, 19 files repaired, and
eight findings plus six out-of-catalogue defects recorded and left for the
orchestrator. The AGENTS rewrite added orientation, a preservation-intent
section and annotated references while keeping every boundary and both
generated blocks byte-identical; `CLAUDE.md` is now exactly `@AGENTS.md`.

Evidence: `docs/logs/2026-08/31-214500-northstar-instruction-rust-audit-closeout.md`
and the recorder's `report.md`/`result.json` under
`.git/.../northstar/rust-quality/audits/convergence-20260831-rust-audit/`.

Retained limitations: the pre-existing `effigy doctor` god-file and
attention-marker findings are unchanged, and the Effigy-generated reference
paths in `AGENTS.md` remain wrong but untouched because they sit inside
generated markers.

## Next Task

Orchestrator review of the worker's exact PR head. Requested changes return to
the same worker; accepted checked work may be merged by the orchestrator.
