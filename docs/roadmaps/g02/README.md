# g02 Roadmaps

`g02` is the active roadmap generation for Convergence.

## Context

`g01` closed as the foundational/research generation (archived on
`archive/g01`). `g02` carried the archive-and-rebuild boundary and now
runs the rebuild improvement program.

## Current State

Three programs are complete:

- rebuild and improvement (`g02.001`-`g02.010`)
- audit hardening (`g02.011`-`g02.018`), from the 2026-07-24 four-part
  audit, sequenced security → data safety → transactional/merge
  correctness → architecture honesty → scale walls → workflow completion
  → TUI spec parity → adversarial tests
- secret substrate (`g02.019`-`g02.020`), client-encrypted individual
  and shared secrets

`g02.021`-`g02.025` are the next suite, laid out 2026-07-25. The first
three are schedulable; the last two are parked on triggers that have not
fired, and are written down so their absence reads as a decision rather
than an oversight.

Scheduled by the operator (2026-07-25): `g02.021` then `g02.023`, both
now complete. `g02.022` followed; its shakedown (22.4) is done and its
release batch (22.5) has a built pipeline but **no release**.

`g02.026` was opened 2026-07-27 out of a 22.4 finding, and blocks the
release. The gate graph is write-once at repo creation, so `promote` —
one of the six contract verbs — cannot be reached by anyone, and the
whole multi-gate design is implemented and unusable. The operator's
call: *"We don't ship until Convergence works consistently without
issues for all use cases end to end."*

`g02.023` opens with a simplification sweep rather than a feature: the
TUI has been built, refactored across four batches, and covered by forty
reducer tests without a human ever driving it, so the first batch uses
it and removes things before anything new is added.

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
- [`012-data-safety.md`](./012-data-safety.md) — complete
- [`013-transactional-and-merge-correctness.md`](./013-transactional-and-merge-correctness.md) — complete
- [`014-architecture-honesty.md`](./014-architecture-honesty.md) — complete
- [`015-scale-walls.md`](./015-scale-walls.md) — complete
- [`016-workflow-completion.md`](./016-workflow-completion.md) — complete
- [`017-tui-spec-parity.md`](./017-tui-spec-parity.md) — complete
- [`018-adversarial-test-hardening.md`](./018-adversarial-test-hardening.md) — complete
- [`019-secret-substrate.md`](./019-secret-substrate.md) — complete
- [`020-shared-secrets.md`](./020-shared-secrets.md) — complete
- [`021-real-identity.md`](./021-real-identity.md) — complete
- [`022-ship-readiness.md`](./022-ship-readiness.md) — in progress (22.1-22.4 complete; 22.5 pipeline built, **release not cut**)
- [`023-tui-completion.md`](./023-tui-completion.md) — complete
- [`024-workflow-profiles.md`](./024-workflow-profiles.md) — parked (needs a design partner)
- [`025-edge-and-scale.md`](./025-edge-and-scale.md) — parked (needs a measured ceiling)
- [`026-gate-administration.md`](./026-gate-administration.md) — ready, and blocks the release

## Next Task

Batch card 26.1 (graph model and impact analysis) under `g02.026`.
