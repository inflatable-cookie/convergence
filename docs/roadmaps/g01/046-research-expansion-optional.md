# 046 - Research Expansion (Optional)

Status: Proposed
Owner: Research
Created: 2026-03-07
Depends on: 045
Vision tags: `RESEARCH`, `ARCH`

## Target Envelope

| Target | Envelope | Outcome Expectation |
| --- | --- | --- |
| `RESEARCH` extended coverage | Additional specimen dossiers and value tracks as needed | Research covers additional problem domains |
| `ARCH` deeper foundations | Specialized tracks inform specific architecture areas | Detailed design has research backing |

## Goal

Optional expansion phase for additional research based on Phase 1-3 findings and emerging Convergence needs. Pick tracks from this roadmap based on prototype feedback and architecture gaps.

## Proposed Additional Specimen Dossiers

**Tier 2 specimens (if gaps identified):**
- `fossil` — Integrated philosophy (wiki, bugs, VCS in one package)
- `pijul` — Patch-based, commutative changes, formal theory
- `sapling` — Meta's scale solution, stacked commits model

**Tier 3 specimens (historical context):**
- `subversion` — Understand what Git replaced
- `darcs` — Patch theory predecessor to Pijul

**Tier 4 (collaboration platforms):**
- GitHub/GitLab/Bitbucket — How platforms extend VCS
- Phabricator/Phorge — Differential, stacked diffs workflow
- Gerrit — Patch-based review model

## Proposed Additional Value Tracks

Priority based on Convergence roadmap needs:

**Track 4: Large Binary and Asset Workflows**
- Compare: Git LFS, Git annex, Perforce binary handling, Plastic semantic diff
- Focus: Game/VFX industry patterns, Unity/Unreal integration

**Track 5: Workspace State Management**
- Compare: Git stash, Mercurial shelve, Jujutsu working commits
- Focus: WIP preservation, multiple contexts, restore semantics

**Track 6: Server Authority Models**
- Compare: Git (distributed with upstream), Perforce (centralized), Radicle (p2p)
- Focus: Offline capability, identity, consensus

**Track 7: Review and Approval Workflows**
- Compare: GitHub PRs, GitLab MRs, Phabricator Differential, Gerrit
- Focus: Pre-commit vs. post-commit review, stacked changes

**Track 8: Identity and Provenance**
- Compare: Git signatures, Sigstore, attestation frameworks
- Focus: Cryptographic identity, SBOM integration, audit trails

**Track 9: Permissions and Access Control**
- Compare: Perforce protections, Gitolite, GitHub/GitLab permissions
- Focus: Path-based access, branch protection, fine-grained capabilities

**Track 10: CI/CD Integration Points**
- Compare: Webhooks, event models, required checks
- Focus: Gating, build provenance, reproducibility

## Execution Model

This is a pick-and-choose roadmap. Based on Phase 045 outcomes:

1. If Memo 1 (snap semantics) needs more data → do Track 5
2. If Memo 2 (gates) reveals gaps → do Tracks 7 or 10
3. If Memo 3 (superpositions) needs validation → do Tier 2 dossiers
4. If game/VFX use case emerges → do Track 4
5. If decentralization becomes priority → do Track 6

## Tasks (Template)

### A) Selected Specimen Dossiers

- [ ] Research system internals
- [ ] Document architectural bets
- [ ] Record strengths and pain points
- [ ] Extract Convergence lessons

### B) Selected Value Tracks

- [ ] Compare ≥3 systems
- [ ] Identify patterns and failures
- [ ] Write Convergence implications
- [ ] Create source maps

### C) Translation Memos

- [ ] Convert findings to recommendations
- [ ] Specify outcomes
- [ ] Feed into architecture

## Exit Criteria

- Selected dossiers and tracks completed
- Relevant translation memos produced
- Architecture docs updated as needed

## References

- `docs/research/value-tracks/README.md` — Full track list (16 defined)
- `docs/research/specimen-dossiers/README.md` — Dossier tiers
- Results from Roadmap 045
