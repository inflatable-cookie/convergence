---
title: Installed Rust package canary worker handoff
kind: northstar-handoff
handoff_mode: worker-pr-loop
worker_mode: implementation
dispatch_authority: orchestrator
handoff: single-file-path-only
status: review-only-pr
owner: repo maintainers
created: 2026-09-03
updated: 2026-09-03
handoff_path: /Users/tom/Dev/projects/convergence/docs/handoffs/20260903-082521-installed-rust-package-canary.md
base_required: pushed-main
tags: [coordination, handoff, worker, pr, language-quality]
---

## What This Thread Was Doing

Northstar's Rust language-quality package is published and pinned. This worker
owns its real Convergence consumer canary under `g02.031/102`.

## Why It Matters

Northstar cannot remove its frozen embedded Rust payload until a real consumer
proves the installed package preserves workflow scope, repository policy, and
pre-extraction evidence compatibility. The original Convergence audit ledger
is unavailable in the current common Git directory, so the worker must state
that limit rather than claim to reread it.

## Current State

- **Repository:** `/Users/tom/Dev/projects/convergence`
- **Planning branch:** `main`
- **Planning base commit:** `1f05db1e507aa67f73a68eccc2325e23dfc1d478`
- **Pushed main verification:** start from the pushed commit containing this handoff and confirm `HEAD == origin/main`.
- **Planning checkout:** clean at dispatch.
- **Worker mode:** implementation worker dispatched by the orchestrator.
- **Planning artifacts included at the base:** `g02.031`, card 102, opening log, and this handoff.
- **Worker branch:** `worker/installed-rust-package-canary`
- **Worker worktree:** Paseo launcher-owned worktree.
- **Worktree creation command:** Paseo `branch-off` from `origin/main`.
- **Worker worktree policy:** launcher worktree first; never create a second.
- **Required sibling worktree links:** `northstar` from `/Users/tom/Dev/projects/northstar`; `northstar-language-packs` from `/Users/tom/Dev/projects/northstar-language-packs`; both beside this worktree.
- **Active spec lane:** none; this is a contract/card-bound maintenance proof.
- **Roadmap milestone:** `docs/roadmaps/g02/031-northstar-rust-package-canary.md`.
- **Ready cards, in order:** `docs/roadmaps/g02/batch-cards/102-installed-rust-package-canary.md` only.
- **Allowed runway:** installed Rust consumer proof and evidence-only closeout.
- **Remaining card budget:** one card; stop after review-only PR.
- **Dispatch topology:** sole ready lane.
- **Parallel safety check:** no other Convergence implementation lane is ready; operator-gated product lanes remain untouched.
- **Surfaces this lane owns:** card 102, g02.031, this handoff, one dated canary log, and affected Convergence front doors.
- **Integration ownership:** this worker owns its bounded evidence closeout.
- **Merge ordering:** one Convergence PR; orchestrator exact-head review then merge.
- **Canonical refs:** `AGENTS.md`; `docs/contracts/001-working-rules.md`; Rust profile/deviations; Northstar contract 004 and spec 034 from the read-only sibling.
- **Review oracle:** seven rows in `g02.031`.
- **Model capability profile:** token-heavy mechanical audit/evidence lane with settled boundaries; choose a fast low-cost or day-to-day profile.
- **Frontier-worker justification:** none.
- **Tool/runtime restrictions:** siblings read-only; no product repair, release, CI, or embedded-payload removal.
- **Required validation:** card 102 acceptance plus `effigy validate`, `effigy qa:docs`, `effigy qa:northstar`, `git diff --check`.
- **PR base/head:** `main` <- `worker/installed-rust-package-canary`.
- **PR URL:** https://github.com/inflatable-cookie/convergence/pull/4
- **Tested head:** `b71b532c4fa09bda4b37d24463430391ee3e90f1`
- **Review state:** awaiting orchestrator review.
- **Merge path:** orchestrator after accepted current-head review and passing checks.

## Boundaries

- **In scope:** all ordered work in card 102, using disposable consumer materializations for workflow execution and writing only evidence/docs to this branch.
- **Out of scope:** Convergence product/source repair; Northstar or package-source edits; rule, profile, evidence-schema, MSRV, release, workflow, or CI changes.
- **Outcome shape:** evidence-only canary PR. Record findings as retained evidence; do not repair them.
- Never mutate either sibling checkout. Never let audit metadata or setup state pollute the shared primary checkout.
- Do not merge the PR.

## Important Context

- **Planning lineage:** Northstar `g02.048/119`; core PR 27 merge `256d0f7`; accepted package-source merge `56b2e11`.
- **Why this card is ready:** registry identity, generic lifecycle, installed-route proof, Rust-only inventory, and bounded fallback passed independent review.
- **Decisions and preferences:** Convergence is the operator-selected consumer; evidence must not become product repair authority.
- **Open tensions:** repository-native tests may expose existing environmental limits; reproduce and classify them without widening scope. The original `convergence-20260831-rust-audit` ledger is unavailable; use the representative Convergence ledger proof and say so.
- **Report after:** pushed review-only PR with exact head, seven-row oracle map, hashes, validation, and limits.
- **Report to:** originating orchestrator through Paseo.

## Suggested Next Move

Canary and evidence closeout are complete. Review the worker PR at its exact
tested head; do not start Northstar card 120.

## Completion Protocol

### Before you start

Accept only a clean registered non-`main` launcher worktree. Fetch with bounded
non-interactive SSH, confirm the recorded planning base is an ancestor, and
load this handoff from `HEAD`. Verify both sibling links resolve to their named
primary checkouts before broad reads. Stop on dirty state, link conflict, or
handoff mismatch; never create a second launcher worktree.

### While you work

Follow card 102 exactly. Use disposable Convergence materializations for setup,
authoring, and audit execution. Keep siblings read-only and keep product code
unchanged. Record, but do not repair, newly observed findings. Stop on policy,
workflow, identity, evidence, or product-contract change.

### When the assigned runway is complete

Falsify all seven oracle rows, run the required validation, reconcile card,
roadmap, log, handoff, and front doors, then push and open a review-only PR.
Report the URL and exact tested head. Do not start Northstar card 120.

### Review and merge path

The orchestrator posts an exact-head verdict. If changes are requested, repair
only those findings on this branch and notify the orchestrator. Accepted head,
passing checks, and mergeability authorize orchestrator merge without another
operator prompt.

### Handoff closeout

Close only card 102 and g02.031 when the canary evidence is honest. Product
lanes and retained audit findings remain unchanged.
