# 027 Retention Policy

Status: ready
Updated: 2026-07-24
Roadmap: `g02.008`
Spec: `docs/specs/008-releases-retention-and-gc.md`

## Objective

Retention as control-plane config: what history each repo/channel keeps,
shared between server GC (8.3) and client thinning.

## In Scope

- model: `RetentionPolicy { keep_releases_per_channel: Option<u32>,
  keep_bundles_per_gate: Option<u32>, keep_publication_days: Option<u32> }`
  stored per repo in the control plane (admin capability to set)
- HTTP + client + CLI: `retention show/set`
- policy evaluation pure functions (what would be dropped) — consumed by
  8.3 GC mark; no deletion in this batch
- client thinning tiers (6.1) become configurable via workspace config
  (`retention` block already in `WorkspaceConfig` — wire it through)
- tests: policy CRUD + authz, evaluation functions over synthetic
  histories, thinning honors configured tiers

## Out Of Scope

- actual GC deletion (8.3), verify (8.4)

## Acceptance Criteria

- retention config round-trips; evaluation pure and tested; client
  thinning configurable

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- retention interacts with lineage in murky ways — doc 17 first

## Next Task

On completion, open the Batch 8.3 GC card.
