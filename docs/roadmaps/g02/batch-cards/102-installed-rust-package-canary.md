# 102 Installed Rust Package Canary

Status: complete
Updated: 2026-09-03
Roadmap: `g02.031`

## Objective

Run a real Convergence consumer canary against Northstar's official
`@northstar/rust-quality` `0.1.0` pin and return reviewable evidence without
repairing Convergence product code.

## Ordered Work

1. Materialize an isolated installed copy of Northstar core at registry
   version `1.4.0`. Acquire the accepted Rust package at source commit
   `56b2e1107b80f369807cff88e1b0253df035c700`, tree
   `sha256:e5cf9c5da4a30c0f5164f2ea0c5e9d87d544c0c32f09f3c139a386c56154dba0`,
   and manifest
   `sha256:dd71d04efd67cc7805f417a79666dd920ea1811ee252d941108dfbeca8aab612`.
   Keep package-source and Northstar siblings read-only.
2. Snapshot tracked Convergence bytes plus the profile, deviations, activation
   block, and existing v2 evidence. Exercise setup and both declared workflows
   from the lifecycle `installed_path`, with the package as task/instruction
   source and Convergence as consumer target.
3. Prove everyday authoring remains changed-tranche-only using a controlled
   disposable Convergence materialization. It must not initialize or claim a
   repository-wide explicit audit.
4. Run one fresh repository-scope explicit audit from the installed package in
   a disposable Convergence materialization. Finalize honest findings and
   limitations, but make no repair wave and no product-code edit.
5. Re-read pre-extraction v2 evidence with the installed engine. Assert the
   profile, deviations, activation, evidence meaning, and tracked consumer
   bytes remain compatible and that retained package state contains no
   TypeScript/Svelte payload.
6. Force the official-acquisition failure path and capture the exact bounded
   frozen-Rust fallback notice. Keep host `stopped` distinct from fallback.
7. Add one dated evidence log, close this card and `g02.031`, update affected
   front doors, validate, and open a review-only PR.

## Scope

- Package lifecycle and installed-route evidence against Convergence.
- Disposable local materializations for authoring/audit execution.
- This card, `g02.031`, one dated log, the dispatch handoff, and directly
  affected Convergence front doors.

## Out Of Scope

- Product repairs, new Rust findings as implementation authority, rule or
  profile changes, MSRV decisions, public API/wire/persistence changes,
  releases, CI, and edits to either sibling repository.

## Acceptance

- Exact registry, package-tree, manifest, receipt, and installed-tree identities
  agree before routing.
- Both workflows route from the same verified installed package and keep their
  distinct scope.
- The explicit audit finalizes through the installed Cargo-native engine with
  all units and retained limitations accounted for and no repair plan applied.
- Existing profile and deviation hashes remain
  `5049d861115f819db5368dcd9ab2dc45381d1be6c5ae3c9947aa1e595fc281a4` and
  `d6d876aeb6e70da9fec368201350b6d16f345a7363309dde4169284c51c2fcd0`.
- Existing pre-extraction evidence remains readable; consumer tracked bytes are
  unchanged outside this lane's documentation; installed inventory is Rust-only.
- Forced fallback names `@northstar/rust-quality@0.1.0`, the stop reason, and
  the frozen embedded Rust payload.
- `effigy validate`, `effigy qa:docs`, `effigy qa:northstar`, and
  `git diff --check` pass or accurately record an unchanged baseline failure.

## Stop Conditions

- Package routing changes Convergence policy, rule meaning, evidence schema,
  product behavior, or tracked source.
- The installed package identity cannot be reproduced exactly.
- Everyday and explicit workflow scope cannot be distinguished honestly.
- A finding appears to need repair; record it and stop that repair path.
- Validation exposes a new product or contract decision.

## Review Oracle

Use the seven rows in `g02.031`. The PR must map each row to executable or
hash-based evidence and disclose every unavailable selector or baseline failure.

## Outcome

The canary ran from the committed worker handoff. The exact registry, package
tree, manifest, receipt, and installed-tree identities matched; both declared
workflows routed through that installed package; setup preserved the existing
policy and activation; and everyday authoring stayed changed-tranche-only.
The installed Cargo-native engine finalized a repository-scope audit with all
six units complete, no findings or repair plans, and retained limitations.
Pre-extraction evidence remained readable, the retained package inventory was
59 Rust-only files, and the forced host stop stayed distinct from the exact
bounded frozen-Rust fallback notice. No Convergence product or sibling source
changed. Validation results and the collector-local CLI integration-test
failure are recorded in the dated canary log.

## Next Task

Review the review-only PR at the tested head. Do not merge from the worker
lane.
