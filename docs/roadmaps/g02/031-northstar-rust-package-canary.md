# 031 Northstar Rust Package Canary

Status: complete
Owner: repo maintainers
Created: 2026-09-03
Updated: 2026-09-03

## Objective

Prove Convergence can use the official installed `@northstar/rust-quality`
package for everyday authoring and explicit audit without changing its Rust
policy, evidence meaning, product code, or existing validation boundary.

## Governing Authority

- `AGENTS.md`
- `docs/contracts/001-working-rules.md`
- `docs/contracts/rust-quality-profile.json`
- `docs/contracts/rust-quality-deviations.json`
- Northstar contract 004 and spec 034 at registry version `1.4.0`
- [`batch-cards/102-installed-rust-package-canary.md`](./batch-cards/102-installed-rust-package-canary.md)

## Runway

- Card 102 only: install, route, exercise both workflows, preserve consumer
  authority, record evidence, and stop for review.

## Boundaries

- Evidence-only maintenance lane. Do not repair product code or retained audit
  findings.
- No release, workflow, wire, persistence, object-identity, MSRV, or public API
  change.
- Do not edit Northstar or `northstar-language-packs`; consume their accepted
  immutable identities read-only.
- Card 102 does not authorize Convergence product execution or close either
  operator-gated product lane.

## Acceptance

- The official Rust package is acquired and routed through the installed core
  contract at the pinned identity, with no TypeScript payload retained.
- Everyday authoring stays changed-tranche-only; explicit audit stays an
  explicit repository-scope workflow.
- Existing profile, deviations, activation, and v2 evidence remain readable and
  byte-compatible.
- Forced acquisition failure emits the bounded frozen-Rust notice without
  turning a host `stopped` result into package success.
- Tracked consumer bytes stay unchanged until this lane writes its own planning
  and evidence closeout.
- Repository validation passes, or a pre-existing failure is reproduced and
  recorded without widening the lane.

## Outcome

Card 102 completed the installed consumer canary in one disposable Convergence
materialization. The pinned package identity, both workflow routes, setup,
changed-tranche authoring, repository-scope explicit audit, pre-extraction
evidence migration, Rust-only inventory, and visible fallback all held. The
audit finalized with six units, no findings or repairs, and explicit
limitations; no Convergence product or sibling source changed.

## Review Oracle

| Invariant | Smallest counterexample | Expected stop or proof |
| --- | --- | --- |
| Installed identity is exact | One installed byte or receipt digest differs from registry `1.4.0` | Route stops before package instructions execute |
| Workflows stay distinct | Everyday authoring initializes a repository audit or explicit audit shrinks to changed files | Scope evidence rejects the run |
| Consumer owns policy | Setup rewrites the existing profile, deviations, or activation block | Canary stops with before/after hashes |
| Evidence survives extraction | The installed engine cannot read or extend a pre-extraction v2 ledger | Compatibility proof fails before closeout |
| Package stays independent | TypeScript/Svelte content appears in retained state | Inventory proof fails |
| Fallback stays visible | Host acquisition stops and embedded Rust runs without the versioned notice | Forced-failure proof fails |
| Product remains untouched | Any Rust/product file changes during the evidence run | Diff attribution fails |

## Next Task

Review the card 102 review-only PR. No product repair or Northstar card 120 is
implied by this completed maintenance lane.
