# Installed Rust Package Canary

Date: 2026-09-03
Roadmap: `g02.031`
Card: `102`
Status: complete; review-only PR open

## Result

The committed worker handoff was executed in one registered Paseo worktree.
The proof used disposable Convergence materializations and read-only Northstar
siblings. No Convergence product source, Northstar source, package source,
release, CI, MSRV, rule, profile, or evidence-schema surface changed.

The pinned inputs were Northstar core `04a7ee22b9bc9863b5dc68e7ea50cc1eeec6aa9f`
at registry version `1.4.0`, package source commit
`56b2e1107b80f369807cff88e1b0253df035c700`, and package
`@northstar/rust-quality@0.1.0`. The installed package contained 59 files and
no TypeScript, TSX, or Svelte payload.

## Review oracle

| Invariant | Evidence | Result |
| --- | --- | --- |
| Installed identity is exact | Local activation receipt `sha256:ec829b1309c70cebae0f31fe4e7b351e0a8b697b3d0f113f843a06e79ab7446e`; tree `sha256:e5cf9c5da4a30c0f5164f2ea0c5e9d87d544c0c32f09f3c139a386c56154dba0`; manifest `sha256:dd71d04efd67cc7805f417a79666dd920ea1811ee252d941108dfbeca8aab612`; installed prover passed | exact |
| Workflows stay distinct | `resolve` routed both `everyday_authoring` and `explicit_audit_repair` to the same installed path; authoring changed only one controlled source file and created no audit ledger; explicit audit was repository scope | held |
| Consumer owns policy | Installed `rust-quality:setup` passed; profile `5049d861115f819db5368dcd9ab2dc45381d1be6c5ae3c9947aa1e595fc281a4` and deviations `d6d876aeb6e70da9fec368201350b6d16f345a7363309dde4169284c51c2fcd0` were preserved | held |
| Evidence survives extraction | Installed prover passed cross-boundary v2 migration and byte preservation; installed engine finalized the new ledger | held |
| Package stays independent | Retained installed inventory was 59 Rust-only files; package self-check and decoy-catalogue checks passed | held |
| Fallback stays visible | Official acquisition returned host `stopped` with the transport-capability notice; the separate fallback emitted the versioned frozen-Rust notice | held |
| Product remains untouched | Final audit `changed_files: []`; disposable consumer tracked bytes stayed clean except the intentional authoring fixture change; worker diff is documentation-only | held |

## Installed audit receipt

The installed Cargo-native engine ran `inspect`, `plan`, `init`, six unit
assessments, native evidence collection, six completions, and `finalize` for
`convergence-installed-rust-canary`. The final receipt is repository scope,
six units, no findings, no repair plans, and status `degraded` because the
canary does not replace the prior semantic audit and records unavailable native
evidence classes explicitly.

Collection recorded 36 evidence rows: four passed, one warning, one failed,
and 30 unrun rows. In that collector invocation, `cargo test -p converge-cli
--all-features` failed four `resolve_loop_e2e` publish/sync assertions against
the unchanged materialized main tree. The model warning is the test name
`naming_the_target_yourself_is_not_a_divergence_warning`, not a compiler
diagnostic. The configured validation path subsequently passed all 364 tests,
so this collector-local observation is retained without widening the lane. No
failure produced a repair plan.

## Fallback and route details

The forced official acquisition stop was kept separate from local operator
activation. The exact fallback line was:

```text
@northstar/rust-quality@0.1.0 unavailable (workflow explicit_audit_repair for @northstar/rust-quality@0.1.0 stopped: host transport capability unavailable for source type none; manual or local-path installation route required); using the frozen embedded Rust payload during the bounded overlap window
```

The installed package self-proof passed Spec-034 tree identity, source parity,
cross-boundary migration, setup, decoy isolation, engine build, and tamper
rejection. The installed engine receipt was current and matched source and
embedded payload `2b75b0866e3bedf99c133e53cb742c284715fb1f10f589358ce2a91331571157`.

## Validation

`effigy tasks` and `effigy test --plan` passed. `effigy validate` passed 364
tests with 4 skipped; `effigy qa:docs`, `effigy qa:northstar`, and
`git diff --check` passed. Review-only PR: [#4](https://github.com/inflatable-cookie/convergence/pull/4),
tested head `b71b532c4fa09bda4b37d24463430391ee3e90f1`.

## Next Task

Review the current worker head, then let the orchestrator decide whether to
merge. Do not start Northstar card 120 or any product repair lane.
