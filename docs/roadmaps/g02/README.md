# g02 Roadmaps

`g02` is the active roadmap generation for Convergence (closing posture).

## Context

`g01` closed as the foundational/research generation (archived on
`archive/g01`). `g02` carried the archive-and-rebuild boundary and the
full rebuild improvement program through identity, ship readiness, gate
administration, TUI usability, semver releases, and the candidate rename.

## Current State

Programs complete through `g02.029` (candidate rename). Open lanes:

- **`g02.022` ship readiness** — 22.1–22.4 complete; batch 22.5 (card 091)
  has a built release pipeline but **no release cut** (operator-gated)
- **`g02.027` TUI usability** — cards 096 (frame/navigation), 097 (guidance),
  100 (decisions on screen) complete; operator cold-drive verdict pending
  before formal closeout. Root redesign and semver tile work landed early
  during 27.3/28.

Parked on triggers: `g02.024` workflow profiles, `g02.025` edge/scale.

Independent maintenance lane:

- **`g02.030` Northstar instruction and Rust quality audit** — card 101 ready

Product execution still waits on operator direction.

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
- [`022-ship-readiness.md`](./022-ship-readiness.md) — in progress (22.1–22.4 complete; 22.5 gated)
- [`023-tui-completion.md`](./023-tui-completion.md) — complete
- [`024-workflow-profiles.md`](./024-workflow-profiles.md) — parked (needs a design partner)
- [`025-edge-and-scale.md`](./025-edge-and-scale.md) — parked (needs a measured ceiling)
- [`026-gate-administration.md`](./026-gate-administration.md) — complete
- [`027-tui-usability.md`](./027-tui-usability.md) — closing (096, 097, 100 complete)
- [`028-semver-releases.md`](./028-semver-releases.md) — complete
- [`029-candidate-rename.md`](./029-candidate-rename.md) — complete
- [`030-northstar-instruction-and-rust-quality-audit.md`](./030-northstar-instruction-and-rust-quality-audit.md) — in progress (card 101 ready)

## Next Task

Run card 101 in the isolated `g02.030` maintenance lane. Product direction
remains unresolved:

1. **TUI closeout** — cold-drive verdict on `g02.027`; close the roadmap if
   exit criteria are met
2. **First release** — push tag and cut release via `g02.022` batch 22.5 when
   the operator authorises it

Long-horizon sequencing: `generation-index.md` strategic horizons (atlas,
2026-08-17). Do not open `g03` until g02 rollover closeout is complete.
