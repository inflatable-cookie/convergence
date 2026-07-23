# 009 Server Slice

Status: ready
Updated: 2026-07-23
Roadmap: `g02.003`
Spec: `docs/specs/003-rebuild-vertical-slice.md`

## Objective

First real `converge-server`: pluggable embedded storage, control-plane
minimum with enforced authz, and the deterministic publish → bundle →
promote path from architecture doc 14.

## In Scope

- `MetadataStore` / `ObjectStore` traits; embedded impls (SQLite via
  rusqlite or equivalent, sharded local FS)
- control plane minimum: users + capability grants
  (subject × scope pattern × capability), one repo, gate graph storage
- authz context required by construction on every data-plane handler
- partition writer for `(repo, scope, gate)`: publish intake (accept fast,
  coalesce async or synchronously in-process for now), deterministic bundle
  build (Merkle merge; divergent paths become `Superposition` nodes),
  promote with policy check (required approvals from gate graph)
- bundle status lifecycle `building -> ready/failed` visible in records
- unit/integration tests: authz denial, publish -> bundle determinism
  (same inputs, same bundle manifest), superposition creation on divergent
  publishes, promote blocked/allowed

## Out Of Scope

- HTTP surface and client sync (Batch 3.5 exposes this engine over the wire)
- external backends (Postgres/S3)
- GC, releases, edge nodes

## Acceptance Criteria

- server crate builds as a library engine + thin bin; all listed tests green
- two publishes with divergent content for one path produce a bundle whose
  manifest holds a two-variant superposition with per-variant provenance
- no data-plane entry point callable without an authz decision
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- storage trait shape fights the arch-14 partition model — stop and route
  through `docs/architecture/` before coding around it

## Next Task

On completion, open the Batch 3.5 end-to-end sync card.
