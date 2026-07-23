# Architecture

This folder holds the durable Convergence architecture: the concept/object
model and superposition semantics that survived the g01 era.

Convergence is designed for large development organizations first, with the
same architecture usable by solo/small teams via lightweight deployments. The
g01-era implementation did not honor that claim (single-node dev server); the
rebuild architecture (roadmap `g02.002` Batch 2.4) owns closing that gap.

Docs:

- `01-concepts-and-object-model.md` — canonical statement of the object model
  and six-verb contract; other docs reference it rather than restate it
- `04-superpositions-and-resolution.md` — conflict-as-data semantics
- `product-guardrails.md` — terminology and product-shape guardrails

The g01-era architecture set (repo/gates/lanes/scopes detail, operations,
policy, storage, client/server, security, CLI/TUI, interop, gate-graph schema)
is archived on branch `archive/g01` under `docs/architecture/`. Treat it as
evidence, not authority; the rebuild rewrites those surfaces.

Related:

- Origin rationale: `docs/git-podcast/summary.md`
- Lessons and rebuild inputs: `docs/rebuild/`

## Next Task

Feed this spine into the `g02.002` Batch 2.4 rebuild-architecture card.
