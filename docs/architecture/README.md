# Architecture

This folder holds the durable Convergence architecture: the concept/object
model and superposition semantics that survived the g01 era.

Convergence is designed for large development organizations first, with the
same architecture usable by solo/small teams via lightweight deployments. The
g01-era implementation did not honor that claim (single-node dev server); the
rebuild server architecture (doc 14) owns closing that gap.

Docs:

- `01-concepts-and-object-model.md` — canonical statement of the object model
  and six-verb contract; other docs reference it rather than restate it
- `04-superpositions-and-resolution.md` — conflict-as-data semantics
- `product-guardrails.md` — terminology and product-shape guardrails

Rebuild architecture (g02.002 Batch 2.4, 2026-07-23):

- `13-rebuild-workspace-and-crates.md` — workspace layout, crate boundaries,
  salvage migration map
- `14-server-authority-and-distribution.md` — central control plane,
  partitioned data plane, pluggable storage, enforced authz, deterministic
  candidate coalescing; §0 and §7 separate the shipped single-process server
  from the deferred distributed target
- `15-client-and-tui-architecture.md` — CLI as canonical verb surface, TUI
  as thin front-end per the captured UX spec
- `16-sync-protocol-and-chunking.md` — wire contract and FastCDC
  content-defined chunking
- `17-lineage-and-merge-semantics.md` — snap DAG and identity, base-aware
  3-way merge with tombstones, candidate windows, per-gate coalesce
  strategies (g02.005; authoritative over 14/16 where they overlap)
- `18-git-interop.md` — export/import mapping contract, mirror-branch
  rules, coexistence boundaries (g02.009)
- `19-secrets-and-key-management.md` — client-side encrypted secrets:
  threat model, why secrets are not files, key lifecycle, why recipient
  removal is not revocation, and how a secret reaches a process without
  a plaintext file (g02.019/020)

The g01-era architecture set (repo/gates/lanes/scopes detail, operations,
policy, storage, client/server, security, CLI/TUI, interop, gate-graph schema)
is archived on branch `archive/g01` under `docs/architecture/`. Treat it as
evidence, not authority; the rebuild rewrites those surfaces.

Related:

- Origin rationale: `docs/git-podcast/summary.md`
- Lessons and rebuild inputs: `docs/rebuild/`

## Next Task

Build against docs 13-16 in the first rebuild implementation roadmap.
