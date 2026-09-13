# g02 Tasks

`g02` is the active Northstar task generation for Convergence (closing
posture). Each `NNN-<slug>.md` file below is one Northstar task `g02.NNN` —
the sole executable planning unit. Per-batch execution cards (001–102)
were absorbed into their owning tasks during the flattened-task migration;
full card text remains in git history, and inline `(card NNN)` notes are
pointers into that history.

## Context

`g01` closed as the foundational/research generation (archived on
`archive/g01`). `g02` carried the archive-and-rebuild boundary and the
full rebuild improvement program through identity, ship readiness, gate
administration, TUI usability, semver releases, and the candidate rename.
The flattened-task switchover merged in PR #5 and the roadmap-backlog
retirement merged in PR #6; neither added product work.

## Current State

Programs complete through `g02.029` (candidate rename). Open tasks:

- **`g02.022` ship readiness** — 22.1–22.4 complete; the 22.5 release step
  has a built release pipeline but **no release cut** (operator-gated)
- **`g02.027` TUI usability** — frame/navigation, guidance, and
  decisions-on-screen work complete (batch records 096, 097, 100);
  operator cold-drive verdict pending before formal closeout. Root redesign
  and semver tile work landed early during 27.3/28.

Parked on triggers: `g02.024` workflow profiles, `g02.025` edge/scale.

Independent maintenance tasks:

- **`g02.030` Northstar instruction and Rust quality audit** — complete;
  the repository-scope Rust audit and the AGENTS rewrite are delivered
  (batch record 101, PR #3), and its retained findings return to the
  orchestrator
- **`g02.031` Northstar Rust package canary** — complete; the installed
  official package was proved against Convergence without authorizing
  product repair (batch record 102, PR #4)
- **Flattened-task switchover** — complete; one task per file is now the
  canonical `g02` execution model (PR #5)

Product execution still waits on operator direction.

## Tasks

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
- [`027-tui-usability.md`](./027-tui-usability.md) — closing (096, 097, 100 complete; operator verdict pending)
- [`028-semver-releases.md`](./028-semver-releases.md) — complete
- [`029-candidate-rename.md`](./029-candidate-rename.md) — complete
- [`030-northstar-instruction-and-rust-quality-audit.md`](./030-northstar-instruction-and-rust-quality-audit.md) — complete (batch record 101, PR #3)
- [`031-northstar-rust-package-canary.md`](./031-northstar-rust-package-canary.md) — complete (batch record 102, PR #4)

## Queue lifecycle adoption

- [g02.032 Effigy-hosted lifecycle hook](032-adopt-effigy-hosted-lifecycle-hook.md)
  is an operator-approved, configuration-only maintenance lane. It follows its
  declared Queue dependencies and may run without changing product priority.
  Existing next-task text continues to describe product sequencing; this entry
  authorizes no sibling product work.

## Next Task

The flattened-task switchover (PR #5) and roadmap-backlog retirement (PR #6)
are merged; neither changes the product queue. The next product intent
checkpoint is the operator's call among:

1. **Audit follow-up** — promote a bounded set of retained findings into a
   new task in this generation
2. **TUI closeout** — cold-drive verdict on `g02.027`; close the task if
   exit criteria are met
3. **First release** — push tag and cut release via the `g02.022` release
   step when the operator authorises it

Long-horizon sequencing: `generation-index.md` strategic horizons (atlas,
2026-08-17). Do not open `g03` until g02 rollover closeout is complete.
<!-- northstar:lifecycle:begin schema=northstar.lifecycle.projection.v2 digest=sha256:c802836a0adce05a6e9df621cbf58427b8756343af64e7dd7078cce508f77235 -->
| Generation | Disposition | Runway state |
| --- | --- | --- |
| g02 | open | planning_required |
| Task | Status | Stage | Revision | Record digest |
| --- | --- | --- | --- | --- |
| g02.032 | complete | none | 8 | sha256:e31ca216ea4ceafcd6576364f17dfcece94a1e16aa5c3ffa4b486495058965a1 |
<!-- northstar:lifecycle:end -->
