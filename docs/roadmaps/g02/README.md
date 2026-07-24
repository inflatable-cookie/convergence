# g02 Roadmaps

`g02` is the active roadmap generation for Convergence.

## Context

`g01` is complete as the foundational implementation, Northstar alignment, and
research generation. `g02.001` held the post-research pause until a real
boundary was named.

## Current State

`g02.008` (releases, retention, and GC) is active. `g02.007` closed:
lane registry with ACLs, unpublished sync, inbox, verified provenance.
`g02.009`-`g02.010` remain planned.

Ready batch card:
[`batch-cards/027-retention-policy.md`](./batch-cards/027-retention-policy.md)

The paused posture is over. The lane is capture → archive cut → docs spine
restructure → rebuild architecture definition. Batch 2.1 (capture) is
complete; artifacts live in `docs/rebuild/`.

Batch 2.2 (archive cut) complete: `v0-legacy` tag, `archive/g01` branch,
`main` stripped to docs + salvaged lib crate. Batch 2.3 (docs spine
restructure) complete: docs tree reduced to the keeper spine.

## Lanes

- [`001-post-research-execution-planning-gate.md`](./001-post-research-execution-planning-gate.md) — complete
- [`002-archive-and-rebuild-boundary.md`](./002-archive-and-rebuild-boundary.md) — complete
- [`003-rebuild-implementation-vertical-slice.md`](./003-rebuild-implementation-vertical-slice.md) — complete
- [`004-tui-rebuild.md`](./004-tui-rebuild.md) — complete
- [`005-convergence-semantics-revision.md`](./005-convergence-semantics-revision.md) — complete
- [`006-continuous-capture-and-workspace-ux.md`](./006-continuous-capture-and-workspace-ux.md) — complete
- [`007-lanes-and-collaboration.md`](./007-lanes-and-collaboration.md) — complete
- [`008-releases-retention-and-gc.md`](./008-releases-retention-and-gc.md) — active
- [`009-git-interop.md`](./009-git-interop.md) — planned
- [`010-scale-and-transport.md`](./010-scale-and-transport.md) — planned

## Next Task

Execute the ready `g02.008` Batch 8.2 retention-policy card.002` Batch 2.4 rebuild-architecture card.
