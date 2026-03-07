# 043 - Research Baseline Specimen Dossiers

Status: Complete
Owner: Research
Created: 2026-03-07
Depends on: 042
Vision tags: `RESEARCH`, `ARCH`

## Target Envelope

| Target | Envelope | Outcome Expectation |
| --- | --- | --- |
| `RESEARCH` corpus foundation | 5 specimen dossiers completed with source inventory | Comparative research has documented specimens |
| `ARCH` informed design | Evidence-based understanding of existing VCS architectural choices | Convergence design decisions reference real system tradeoffs |

## Goal

Create foundational specimen dossiers for version control systems representing different architectural choices. Establish the comparative baseline that all subsequent Convergence research builds upon.

## Why This Phase Exists

Convergence aims to solve problems Git treats as edge cases. Before designing solutions, we need to understand:
- What architectural bets existing systems made
- What worked at scale and what broke
- What patterns exist for gate workflows, conflict handling, and continuous capture

This roadmap produces the "specimens" that Phase 2 value tracks will synthesize.

## Scope

### In Scope

Complete specimen dossiers for:

1. **Git** (complete the existing draft)
   - Object store internals (packfiles, delta compression)
   - Transport protocols
   - Subtree/submodule design decisions
   - Source inventory completion

2. **Mercurial** (new)
   - Revlog format and manifest structure
   - Phases (draft/public/secret) as precursor to gates
   - Extensions model (evolve, topics, shelve)
   - Comparison with contemporary Git

3. **Perforce Helix Core** (new)
   - Centralized model and depot structure
   - Client views and workspace mappings
   - Streams and branching policy
   - File locking for unmergeable assets
   - Game industry workflow patterns

4. **Plastic SCM** (new)
   - Semantic merge and diff algorithms
   - Visual branching model
   - Binary asset handling
   - Unity integration patterns
   - Xlinks (subrepo) design

5. **Jujutsu** (complete the existing draft)
   - Working-copy-as-commit model
   - Conflict storage and representation
   - Operation log design
   - Git backend compatibility approach

### Out of Scope

- Value track syntheses (Phase 2)
- Translation memos (Phase 3)
- Prototype implementations
- Direct architecture changes without memo review

## Architecture Notes

Dossiers should follow the structure in `docs/research/specimen-dossiers/README.md`:
- Product identity and era context
- Defining architectural bets
- Standout strengths
- Chronic pain points
- Between-release corrections
- Convergence-relevant lessons
- Source inventory

## Tasks

### A) Complete Git dossier

- [ ] Document packfile format and delta chains
- [ ] Document transport protocols (smart HTTP, SSH, git)
- [ ] Add submodule/subtree design analysis
- [ ] Complete source inventory table
- [ ] Review for accuracy and confidence markings

### B) Write Mercurial dossier

- [ ] Research revlog format and storage model
- [ ] Document phases concept (draft/public/secret)
- [ ] Analyze extensions architecture
- [ ] Compare head-to-head with Git (same era, different choices)
- [ ] Document why Git won (network effects, GitHub, or technical?)

### C) Write Perforce Helix Core dossier

- [ ] Research depot and client view syntax
- [ ] Document centralized model tradeoffs
- [ ] Analyze streams vs. branches
- [ ] Document exclusive checkout (locking) mechanism
- [ ] Record game industry adoption patterns

### D) Write Plastic SCM dossier

- [ ] Research semantic merge algorithms
- [ ] Document visual branching UX
- [ ] Analyze binary diff capabilities
- [ ] Document Xlinks design
- [ ] Record Unity integration patterns

### E) Complete Jujutsu dossier

- [ ] Document working-copy-as-commit implementation
- [ ] Research conflict representation format
- [ ] Document operation log storage
- [ ] Analyze Git interop design
- [ ] Document lessons for Convergence snap model

### F) Cross-review

- [ ] Verify all dossiers have consistent format
- [ ] Check source quality (prefer primary sources)
- [ ] Flag uncertain claims explicitly
- [ ] Ensure Convergence-relevant lessons are extracted

## Exit Criteria

- 5 specimen dossiers complete in `docs/research/specimen-dossiers/`
- Each dossier has source inventory with confidence markings
- Each dossier answers: what to adopt, study, or reject
- Research README updated with progress

## Follow-on Roadmaps

- Roadmap 044: Research Core Value Track Syntheses

## References

- `docs/research/README.md` — Research program overview
- `docs/research/specimen-dossiers/README.md` — Dossier structure
- `docs/research/GETTING-STARTED.md` — Execution guidance
