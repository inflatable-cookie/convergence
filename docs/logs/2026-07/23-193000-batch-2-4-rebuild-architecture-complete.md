# 2026-07-23 19:30:00 BST - Batch 2.4 Rebuild Architecture Complete; g02.002 Closed

Roadmap: `g02.002`

## Summary

Promoted the rebuild architecture and closed the archive-and-rebuild boundary
roadmap. All four `g02.002` batches are complete.

## Decisions (operator, 2026-07-23)

- Server authority: central control plane, partitioned
  `(repo, scope, gate)` data plane, edge caches without authority.
  Federation explicitly rejected with rationale (gates are the product's own
  convergence mechanism; no cross-site authority merge semantics).
- Deployment: one binary, pluggable storage traits — embedded SQLite +
  local FS for light deployments, Postgres + S3-compatible for orgs.

## Changes

- added `docs/architecture/13-rebuild-workspace-and-crates.md`,
  `14-server-authority-and-distribution.md` (deepest of the set: consistency
  model, enforced authz, deterministic bundle coalescing, GC, failure
  posture), `15-client-and-tui-architecture.md`,
  `16-sync-protocol-and-chunking.md` (FastCDC replaces fixed-block)
- architecture README indexes the new set
- closed card `005-rebuild-architecture.md`, roadmap `g02.002`, and spec
  `002-archive-and-rebuild-boundary.md`; front doors advanced to `g02.003`
  planning

## Validation

- `effigy qa:docs` / `effigy qa:northstar`

## Next Task

Compile the first rebuild implementation roadmap (`g02.003`) from
architecture docs 13-16.
