# Convergence Research Master Index

Status: Active
Last updated: 2026-03-07
Purpose: Navigate from architecture or implementation questions to the most relevant research artifacts.

## Quick Reference: Architecture Doc -> Research

| Architecture Doc | Primary Memo(s) | Value Track(s) | Dossiers | Prototype / Validation |
| --- | --- | --- | --- | --- |
| `01-concepts-and-object-model.md` | 001, 003 | continuous capture, conflict preservation | Git, Jujutsu, Mercurial | snap capture prototype, superposition architecture update |
| `02-repo-gates-lanes-scopes.md` | 002 | gate-based workflows | Perforce, Plastic SCM, GitHub-style platform patterns | gate chain prototype |
| `03-operations-and-semantics.md` | 001, 002, 003 | continuous capture, gate workflows, conflict preservation | Git, Jujutsu, Perforce | snap + gate workflow validation |
| `04-superpositions-and-resolution.md` | 003 | conflict preservation | Jujutsu, Pijul, Perforce | collaborative resolution UX, storage benchmark |
| `05-policy-model-and-phase-gates.md` | 002 | gate-based workflows | Perforce, protected-branch patterns | policy DSL and gate workflow prototype |
| `06-storage-and-data-model.md` | 001, 003 | continuous capture, conflict preservation | Git, Mercurial, Jujutsu | content-addressing and storage-overhead validation |
| `07-client-workspace-architecture.md` | 001 | continuous capture | Jujutsu, editor auto-save patterns, Git | snap capture UX study |
| `08-server-authority-architecture.md` | 002 | gate-based workflows | Perforce, GitHub/GitLab-style enforcement patterns | server-authoritative gate checks |
| `09-security-identity-and-permissions.md` | 002 | gate-based workflows | server-authority and approval precedents | approval and permissions validation |
| `10-cli-and-tui.md` | 001, 002, 003 | all three core tracks | Git, Jujutsu, Perforce | snap history, promote flow, superposition UX |
| `11-interop-and-migration.md` | 001, 003 | continuous capture, conflict preservation | Git, Mercurial, Plastic SCM | migration-path exploration |
| `12-gate-graph-schema.md` | 002 | gate-based workflows | Perforce streams, CI/CD gate graphs | gate DAG follow-on after linear prototype |
| `prototype-snap-capture.md` | 001 | continuous capture | Jujutsu, editors, Git | active prototype |
| `prototype-gate-chain.md` | 002 | gate-based workflows | Perforce, GitHub branch protection, CI gating | active prototype |

## By Convergence Concept

| Concept | Start Here | Supporting Research |
| --- | --- | --- |
| `snap` | Memo 001 | continuous-capture-vs-explicit-commit, Git, Jujutsu, Mercurial |
| `gate` | Memo 002 | gate-based-workflows, Perforce, platform policy precedents |
| `superposition` | Memo 003 | conflict-preservation, Jujutsu, Pijul, Perforce |
| `publish -> bundle -> promote` flow | Memo 002 + architecture 02/03/05/12 | gate-based-workflows, Perforce, protected-branch patterns |
| local capture vs. server authority | Memo 001 + Memo 002 | continuous capture, gate workflows, Jujutsu, Perforce |

## By Prototype or Validation Work

| Prototype / Validation | Validates | Related Research |
| --- | --- | --- |
| snap capture prototype | automatic capture semantics, message timing, storage overhead | Memo 001, Track 1, Git, Jujutsu, Mercurial |
| gate chain prototype | server-authoritative gate policy and explicit promotion flow | Memo 002, Track 2, Perforce, branch-protection precedents |
| superposition architecture implementation | conflict-as-data structure and resolution lifecycle | Memo 003, Track 3, Jujutsu, Pijul |
| storage benchmark | snap volume and superposition variant cost | Memo 001, Memo 003, Git, Mercurial |
| collaborative resolution UX | team handling of unresolved superpositions | Memo 003, Track 3, Jujutsu, Pijul |

## By System Dossier

| System | Studied For | Key Research Contribution |
| --- | --- | --- |
| Git | object store, explicit capture, staging overhead | baseline incumbent, explicit commit tradeoffs |
| Mercurial | revlog and phases | alternate mutability and publication model |
| Perforce Helix Core | centralized authority, streams, locking | strongest gate/promotion precedent |
| Plastic SCM | hybrid workflows, semantic merge | visual branching and asset-aware workflow cues |
| Jujutsu | working copy as commit, conflicts-as-data | strongest precedent for snaps and superpositions |

## Maintenance Rule

- Update this index when a new translation memo, prototype, or architecture doc becomes part of active implementation work.
- Prefer durable doc references over ad hoc summaries.

## Next Task

Expand this index when roadmap g01.046 opens new research tracks or when additional prototype work creates new research entry points.
