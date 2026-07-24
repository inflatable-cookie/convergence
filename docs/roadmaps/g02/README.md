# g02 Roadmaps

`g02` is the active roadmap generation for Convergence.

## Context

`g01` closed as the foundational/research generation (archived on
`archive/g01`). `g02` carried the archive-and-rebuild boundary and now
runs the rebuild improvement program.

## Current State

The g02.005-g02.010 improvement program is **complete**. The 2026-07-24
four-part audit (server security, client/sync/git correctness, UX,
architecture/docs/tests) produced the hardening program `g02.011`-
`g02.018`, sequenced security → data safety → transactional/merge
correctness → architecture honesty → scale walls → workflow completion
→ TUI spec parity → adversarial test hardening.

`g02.011` server trust boundaries complete (cards 038-041). Active
roadmap: `g02.012` data safety; next is batch card 12.1.

## Lanes

- [`001-post-research-execution-planning-gate.md`](./001-post-research-execution-planning-gate.md) — complete
- [`002-archive-and-rebuild-boundary.md`](./002-archive-and-rebuild-boundary.md) — complete
- [`003-rebuild-implementation-vertical-slice.md`](./003-rebuild-implementation-vertical-slice.md) — complete
- [`004-tui-rebuild.md`](./004-tui-rebuild.md) — complete
- [`005-convergence-semantics-revision.md`](./005-convergence-semantics-revision.md) — complete
- [`006-continuous-capture-and-workspace-ux.md`](./006-continuous-capture-and-workspace-ux.md) — complete
- [`007-lanes-and-collaboration.md`](./007-lanes-and-collaboration.md) — complete
- [`008-releases-retention-and-gc.md`](./008-releases-retention-and-gc.md) — complete
- [`009-git-interop.md`](./009-git-interop.md) — complete
- [`010-scale-and-transport.md`](./010-scale-and-transport.md) — complete
- [`011-server-trust-boundaries.md`](./011-server-trust-boundaries.md) — complete
- [`012-data-safety.md`](./012-data-safety.md) — planned (active)
- [`013-transactional-and-merge-correctness.md`](./013-transactional-and-merge-correctness.md) — planned
- [`014-architecture-honesty.md`](./014-architecture-honesty.md) — planned
- [`015-scale-walls.md`](./015-scale-walls.md) — planned
- [`016-workflow-completion.md`](./016-workflow-completion.md) — planned
- [`017-tui-spec-parity.md`](./017-tui-spec-parity.md) — planned
- [`018-adversarial-test-hardening.md`](./018-adversarial-test-hardening.md) — planned

## Next Task

Open batch card 12.1 (safe restore) under `g02.012`.
