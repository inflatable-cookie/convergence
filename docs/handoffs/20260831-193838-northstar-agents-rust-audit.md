---
title: Convergence Northstar AGENTS and Rust audit worker handoff
kind: northstar-handoff
handoff_mode: worker-pr-loop
worker_mode: implementation
dispatch_authority: orchestrator
handoff: single-file-path-only
status: ready-to-launch
owner: Northstar orchestrator
created: 2026-08-31
updated: 2026-08-31
handoff_path: /Users/tom/Dev/projects/convergence/docs/handoffs/20260831-193838-northstar-agents-rust-audit.md
base_required: pushed-main
tags: [coordination, handoff, worker, pr, agents, rust, audit]
---

## What This Thread Was Doing

The operator chose Convergence as the first project in a project-by-project
Northstar AGENTS and language-quality audit. No Convergence orchestrator was
active, so this thread opened one bounded maintenance runway and now dispatches
you to complete it.

This is one serial worker lane: run the repository-scope Rust audit first, then
the target-aware AGENTS/CLAUDE optimization, close the planning evidence, and
open one PR. No transcript or second prompt is part of the authority chain.

## Why It Matters

Convergence is nearing the end of its rebuild generation. Its agent guidance
and Rust workspace should be trustworthy before the first release, without
turning a quality pass into product redesign or opportunistic cleanup.

## Current State

- **Repository:** `/Users/tom/Dev/projects/convergence`
- **Planning branch:** `main`
- **Planning base commit:** `aa8e94b5cd325e065b81df367e7320a8455c76b3`
- **Pushed main verification:** local `HEAD` and `origin/main` both resolved to
  the planning base before this handoff commit.
- **Planning checkout:** clean when the handoff was created.
- **Worker mode:** implementation worker dispatched by the orchestrator; this
  handoff activates the worker-only worktree preflight.
- **Planning artifacts at the base:** `g02.030`, card 101, and the opening log.
- **Worker branch:** `worker/northstar-agents-rust-audit`
- **Worker worktree:** Paseo-managed worktree returned by the launcher.
- **Worktree creation:** Paseo `branch-off` from `origin/main`; accept the
  launcher path even when it differs from this branch/slug.
- **Required sibling worktree links:** none.
- **Active spec lane:** none; this maintenance card is governed directly by
  `g02.030` and the repository contracts.
- **Roadmap milestone:** `docs/roadmaps/g02/030-northstar-instruction-and-rust-quality-audit.md`
- **Ready card:** `docs/roadmaps/g02/batch-cards/101-northstar-agents-and-rust-audit.md`
- **Allowed runway:** card 101 only.
- **Remaining card budget:** one card.
- **Dispatch topology:** serial; no other Convergence worker lane is active.
- **Canonical refs:** `AGENTS.md`, `docs/architecture/README.md`,
  `docs/contracts/001-working-rules.md`,
  `docs/contracts/rust-quality-profile.json`, and
  `docs/contracts/rust-quality-deviations.json`.
- **Review oracle:** the invariant table in `g02.030` plus card 101.
- **Model capability profile:** frontier/high-reasoning worker; the audit spans
  public API, async/concurrency, persistence, wire, and security-adjacent code.
- **Tool/runtime restrictions:** use the installed Northstar explicit Rust
  audit and agent-instruction review procedures exactly. Do not edit workflows
  or run release mutations. Do not improvise audit tooling or add it to PATH.
- **Required validation:** finalized Northstar Rust audit recorder evidence,
  target-local agent-instruction check, `effigy qa`, `git diff --check`, and
  focused tests required by applied repair waves.
- **PR base/head:** `main` ← `worker/northstar-agents-rust-audit`
- **PR URL:** pending.
- **Review state:** awaiting worker PR.
- **Merge path:** orchestrator after accepted review of the current head and
  passing required checks.

## Boundaries

- **In scope:** every ordered step and surface in card 101.
- **Out of scope:** release execution, workflow edits, feature work,
  architecture replacement, blanket god-file splitting, broad marker cleanup,
  public/wire breakage, MSRV changes, foreign error-policy choices, and unsafe
  repair without operator direction.
- **Outcome shape:** audit-and-repair. Assess the complete repository scope,
  apply only recorder-authorized repairs, optimize the bounded instruction
  surfaces, update evidence/currentness, and open a PR.
- Existing doctor god-file and attention-marker findings are leads, not repair
  authority.
- `RUST-UNSAFE-001` and `RUST-SLOP-001` remain report-only. A public item is
  not a justified façade merely because it is public, but this audit still
  cannot auto-delete a forwarder.
- Preserve the Northstar Rust activation block, Effigy contract, strict
  continuation rule, workflow/release boundaries, project terminology, and
  human writing style.
- Do not invent architecture, change contracts, widen the roadmap, or choose an
  unresolved API/persistence/security decision.
- Work only in the clean launcher worktree. Never edit or clean the planning
  checkout or unrelated work.
- Do not merge. Merge belongs to the orchestrator after review and checks.

## Important Context

- Product execution is still paused between the TUI cold-drive verdict and the
  operator-gated first release. `g02.030` is an independent maintenance lane.
- The strict Rust profile and empty deviations file already exist. The profile
  names the root manifest plus all five crates.
- `effigy doctor` currently reports 45 god-file findings, four attention
  markers, and a stale graph index. Treat these as pre-existing limitations;
  do not claim the audit introduced or cleared them unless direct evidence says
  so.
- `CLAUDE.md` currently includes `@AGENTS.md` plus generic writing-style
  guidance already owned by `AGENTS.md`. The agent-instruction review must
  decide it through the section-intent map; the default bridge contract is the
  exact one-line reference unless a real Claude-only need exists.
- The agent-instruction mechanical report is evidence, not a prose score.
- Report after the complete Rust assessment if it exposes any operator-decision
  stop; otherwise report when the full PR is ready.
- Report through the active Paseo control plane so the originating orchestrator
  can review and, if needed, return changes to this same agent.

## Suggested Next Move

Run the worker preflight below before broad reads. Then load card 101 and the
Northstar Rust explicit audit mode. Complete the recorder-backed Rust audit
before changing `AGENTS.md` or `CLAUDE.md`; this keeps instruction edits from
moving the audit policy beneath an active case.

## Completion Protocol

### Before you start

1. This handoff's worker metadata activates worker mode. Before broad reads,
   run `git rev-parse --show-toplevel`, `git branch --show-current`,
   `git status --porcelain`, and `git worktree list --porcelain`.
2. If the current root is a clean registered non-`main` worktree, accept it as
   the launcher worktree regardless of generated path or branch differences.
   Record the actual root and branch; do not create another worktree.
3. If the launcher context is dirty, `main`, unregistered, or unusable, stop and
   report it. Do not clean, reset, stash, discard, or silently create a second
   worktree.
4. From the selected worktree, run
   `GIT_SSH_COMMAND="ssh -o ConnectTimeout=10 -o BatchMode=yes" git fetch origin`.
   Confirm `HEAD == origin/main`, confirm the planning base is an ancestor of
   `HEAD`, and confirm this handoff exists in `HEAD`. Load the tracked blob with
   `git show HEAD:docs/handoffs/20260831-193838-northstar-agents-rust-audit.md`.
   If it differs from the absolute dispatch file, stop. The tracked copy is
   canonical.
5. No sibling links are required.
6. Read `AGENTS.md`, card 101, `g02.030`, the governing contracts, and the
   Northstar modes named by the card. Run only the cheap orientation needed by
   those procedures.

### While you work

- Follow Northstar's Rust tool bootstrap, repository discovery/plan/init,
  three-pass assessment, authority derivation, coherent repair waves, evidence
  collection, completion, and finalization contracts. Do not mutate a unit
  before its complete assessment and authorized plan exist.
- Keep report-only, retained, read-only, excluded, and operator-decision
  surfaces unchanged. Use `extend` before a necessary cross-unit caller/test/doc
  change; never extend after mutation.
- After Rust finalization, follow the target-aware AGENTS review procedure:
  run the local audit, read the full instruction chain and project context,
  build the section-intent/disposition map, then make only the bounded
  instruction/evidence changes card 101 allows.
- Stop and notify the orchestrator if scope, policy, a public contract, unsafe
  boundary, MSRV, foreign error signaling, or validation requires a decision
  not already settled by the card.
- Do not quietly turn a finding into a new product or architecture decision.

### When the runway is complete

1. Run the final validation named in `Current State` and all evidence selected
   by applied repair waves.
2. Falsify the diff against every `g02.030` review-oracle row. Reconcile the
   recorder result, diff, card, roadmap, log, and front doors. State every
   retained limitation.
3. Mark card 101 and `g02.030` honestly, update currentness and one dated
   closeout log, and leave product queue state unchanged.
4. Push the worker branch and open a PR against current `main`.
5. The PR body must link the roadmap/card, summarize the AGENTS section map,
   list Rust findings and dispositions, identify repairs and limitations, name
   validation actually run, and state unresolved items.
6. Report the PR URL, exact head SHA, recorder result location, changed files,
   and validation through Paseo. Do not merge.

### Review and merge path

The orchestrator independently reviews the exact PR head against card 101,
`g02.030`, the recorder report, diff, and checks. Same-identity GitHub review is
recorded as a PR comment. If changes are requested, the orchestrator will post
them and explicitly wake this same worker; repair only those in-bounds findings
on this branch. A planning change returns to the orchestrator first.

When the exact reviewed head remains current, required checks pass, the PR is
mergeable into `main`, and no stricter rule or operator pause applies, the
orchestrator may merge without another approval prompt.

### Handoff closeout

Card 101, `g02.030`, the closeout log, and the roadmap/front-door next-task
state must agree. If blocked, record the exact limitation and stop rather than
claiming completion.
