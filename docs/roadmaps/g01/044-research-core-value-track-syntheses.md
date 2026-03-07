# 044 - Research Core Value Track Syntheses

Status: Complete
Owner: Research
Created: 2026-03-07
Depends on: 043
Vision tags: `RESEARCH`, `ARCH`

## Target Envelope

| Target | Envelope | Outcome Expectation |
| --- | --- | --- |
| `RESEARCH` cross-system understanding | 3 value track syntheses completed | Comparative analysis informs Convergence semantics |
| `ARCH` design foundation | Documented patterns for snap, gate, and superposition concepts | Architecture has evidence-based precedents |

## Goal

Synthesize cross-system understanding into value tracks that directly inform Convergence's core semantics: continuous capture (snap), gate-based workflows (promote), and conflict preservation (superposition).

## Why This Phase Exists

Individual system dossiers document what each system does. Value tracks extract patterns across systems:
- What approaches exist for continuous vs. explicit capture?
- How have systems implemented policy enforcement and promotion gates?
- What prior art exists for preserving conflicts as data?

These syntheses enable evidence-based decisions about Convergence's novel semantics.

## Scope

### In Scope

Synthesize three priority value tracks:

**Track 1: Continuous Capture vs. Explicit Commit**
- Compare: Git staging area, Fossil auto-sync, Jujutsu auto-amend, editor auto-save
- Analyze: buildability assumptions, message requirements, visibility
- Question: What is a Convergence `snap`? Is it buildable? Who sees it?

**Track 2: Gate-Based Workflows and Phased Convergence**
- Compare: Perforce streams, GitHub branch protection, GitLab MR approvals, CI gating
- Analyze: policy enforcement points, promotion semantics, rollback
- Question: Where do gates live? How is promotability checked?

**Track 3: Conflict Preservation and Superpositions**
- Compare: Git rerere, Pijul commutative patches, Jujutsu conflict commits, Darcs conflict marking
- Analyze: serialization formats, collaboration on resolution, reopening
- Question: How are conflicts stored? Can they be collaborated on?

### Out of Scope

- Translation memos (Phase 3)
- Lower-priority tracks (large binary handling, authority models, etc.)
- Prototype implementations

## Architecture Notes

Value tracks should follow the method in `docs/research/value-tracks/README.md`:
1. Summarize the shared problem
2. Compare how at least three systems approached it
3. Identify repeat failure patterns
4. Identify promising frontier work
5. Write Convergence implications and prototype needs

## Tasks

### A) Track 1: Continuous Capture vs. Explicit Commit

- [ ] Research Git staging area design rationale
- [ ] Research Fossil auto-sync model
- [ ] Document Jujutsu working-copy-as-commit
- [ ] Survey editor auto-save implementations
- [ ] Compare UX patterns and mental models
- [ ] Identify repeat failures (lost work, "WIP" commit mess)
- [ ] Write Convergence implications for `snap` semantics

### B) Track 2: Gate-Based Workflows

- [ ] Research Perforce streams and promotion model
- [ ] Document GitHub branch protection mechanics
- [ ] Research GitLab MR approval patterns
- [ ] Analyze CI gating implementations
- [ ] Compare policy enforcement locations (client vs. server vs. CI)
- [ ] Identify repeat failures (bypassed gates, late failures)
- [ ] Write Convergence implications for `promote` semantics

### C) Track 3: Conflict Preservation

- [ ] Research Git rerere design and limitations
- [ ] Document Pijul patch-based conflict model
- [ ] Research Jujutsu conflict commit format
- [ ] Investigate Darcs conflict marking
- [ ] Compare serialization approaches
- [ ] Identify collaboration patterns (can conflicts be shared?)
- [ ] Write Convergence implications for `superposition` semantics

### D) Source Maps

- [ ] Create source map 001: Git internals
- [ ] Create source map 002: Stacked commits and differential review
- [ ] Create source map 003: Conflict representation patterns

### E) Cross-cutting Analysis

- [ ] Identify common patterns across tracks
- [ ] Document what no system has tried
- [ ] Flag areas needing prototype validation

## Exit Criteria

- 3 value track syntheses in `docs/research/value-tracks/`
- Each track compares ≥3 systems
- Each track identifies Convergence implications
- Source maps created for primary references

## Follow-on Roadmaps

- Roadmap 045: Research Translation Memos and Architecture Input

## References

- `docs/research/value-tracks/README.md` — Track definitions and method
- `docs/research/GETTING-STARTED.md` — Phase 2 guidance
- `docs/architecture/01-concepts-and-object-model.md` — Core definitions to inform
- `docs/architecture/04-superpositions-and-resolution.md` — Superposition theory
