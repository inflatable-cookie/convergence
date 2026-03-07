# Getting Started with Convergence Research

This document provides a roadmap for executing the Convergence research program based on the skeleton structure now in place.

## What We've Built

The research skeleton follows the Jetstream pattern, adapted for version control and collaboration systems:

```
docs/research/
├── README.md                           # Research program overview
├── GETTING-STARTED.md                  # This file
├── master-index.md                     # Architecture/task -> research navigation
├── research-to-implementation-playbook.md
├── quick-start-checklist.md
├── research-to-architecture-crossref.md
├── gaps-found-during-implementation.md
├── discovery-intake.md                 # Triage policy for signals
├── discovery-triage-log.md             # (create when needed)
├── specimen-dossiers/                  # Per-system specimens
│   ├── README.md
│   ├── git.md
│   └── jujutsu.md
├── value-tracks/                       # Cross-system syntheses
│   └── README.md
├── source-hubs/                        # Curated source maps
│   └── README.md
├── translation-memos/                  # Convergence recommendations
│   └── README.md
└── templates/
    ├── README.md
    └── implementation-decision-record.md
```

## Research-to-Implementation Bridge

Once research starts shaping active delivery, use these files:

1. `master-index.md` — quickest route from an architecture area to the relevant memo, value track, dossier, and prototype
2. `research-to-implementation-playbook.md` — expected workflow for research-aware implementation
3. `quick-start-checklist.md` — short contributor checklist for day-to-day use
4. `research-to-architecture-crossref.md` — memo promotion and architecture gap tracking
5. `gaps-found-during-implementation.md` — capture missing research discovered while building
6. `templates/implementation-decision-record.md` — durable decision traceability when implementation choices matter

## Phase 1: Establish Baseline (Week 1-2)

### Goal
Create enough system dossiers to enable meaningful comparison. Focus on systems that represent different architectural choices.

### Tasks

1. **Complete Git dossier** (already started)
   - Add sections on packfiles, transport protocols, and submodule design
   - Document Git's threading model and concurrency limits

2. **Write Mercurial dossier**
   - Focus on revlog format (different from Git's object store)
   - Document phases (draft/public/secret) as precursor to gate concepts
   - Note extensions model (evolve, topics, etc.)

3. **Write Perforce Helix Core dossier**
   - Focus on centralized model and file locking
   - Document depot/client/stream mappings
   - Record gate/branching policy enforcement
   - Critical for game industry workflows

4. **Write Plastic SCM dossier**
   - Focus on semantic merge and visual branching
   - Document binary handling and Unity integration
   - Note the Xlinks (subrepos) design

### Deliverables
- 5 system dossiers (Git, Mercurial, Perforce, Plastic, plus Jujutsu already started)
- Understanding of: distributed vs. centralized, snapshot vs. changeset, merge vs. lock

## Phase 2: First Syntheses (Week 3-4)

### Goal
Synthesize cross-system understanding into value tracks that inform Convergence semantics.

### Priority Tracks

**Track 1: Continuous Capture vs. Explicit Commit**
- Compare: Git staging, Fossil auto-sync, Jujutsu auto-amend, editor auto-save
- Key question: What does `snap` mean? Is it buildable? Does it have a message?

**Track 2: Gate-Based Workflows**
- Compare: Perforce streams, GitHub branch protection, GitLab MR approvals, CI gating
- Key question: Where does policy live? How are gates enforced?

**Track 3: Conflict Preservation**
- Compare: Git rerere, Pijul commutative patches, Jujutsu conflict commits, Darcs conflict marking
- Key question: Can conflicts be serialized? Collaborated on? Reopened?

### Deliverables
- 3 value track synthesis documents
- Identification of patterns Convergence should adopt vs. avoid

## Phase 3: Translation Memos (Week 5-6)

### Goal
Convert research into actionable Convergence recommendations.

### Priority Memos

**Memo 1: Snap Semantics**
- Define what a `snap` is and isn't
- Recommend UX for continuous capture
- Specify relationship to `publish`

**Memo 2: Gate Policy Model**
- Recommend where policy lives (server-side vs. client-side)
- Define promotability checking
- Suggest policy language or configuration

**Memo 3: Superposition as Data**
- Recommend serialization format for conflicts
- Define resolution semantics
- Suggest collaboration patterns

### Deliverables
- 3 translation memos with clear outcomes (promote/prototype/watch/reject)
- Architecture decisions ready for `docs/architecture/`

## Phase 4: Expansion (Ongoing)

### Additional Specimen Dossiers

**Tier 2 (significant alternatives):**
- Fossil (integrated philosophy, shows different scope)
- Pijul (patch-based, commutative, formal theory)
- Sapling (Meta's scale solution, stacked commits)

**Tier 3 (historical/context):**
- Subversion (understand what Git replaced)
- Darcs (patch theory predecessor)

**Tier 4 (collaboration platforms):**
- GitHub/GitLab (how platforms extend VCS)
- Phabricator/Phorge (differential review model)
- Gerrit (patch-based review)

### Additional Value Tracks

Priority order based on Convergence roadmap needs:
4. Large binary handling (games/VFX use case)
5. Workspace state management
6. Server authority models
7. Review and approval workflows
8. Identity and provenance
9. Permissions and access control
10. CI/CD integration points

## Research Execution Tips

### Source Quality Hierarchy

1. **Primary**: Official docs, source code, design docs, academic papers
2. **Secondary**: Conference talks, engineering blogs with technical detail
3. **Tertiary**: Experience reports, comparison articles
4. **Avoid**: Unsubstantiated claims, marketing materials

### Time Boxing

- **System dossier**: 2-4 hours for initial version
- **Value track synthesis**: 3-6 hours (requires cross-referencing)
- **Translation memo**: 2-3 hours (requires clear recommendation)

### When to Stop Researching

Good enough when you can answer:
- What problem does this system solve?
- What did they prioritize (and what did they sacrifice)?
- What breaks at scale?
- What should Convergence adopt, study, or reject?

## Integration with Development

The research program should feed into Convergence development:

```
Research → Translation Memo → Concept Work / Prototype → Architecture → Roadmap → Implementation
```

Research findings should not directly become implementation — they need to pass through:
1. **Translation memo**: Convergence-specific recommendation
2. **Concept work or prototype**: Detailed design or validation path
3. **Architecture**: System boundaries and invariants
4. **Roadmap**: Prioritized milestone

When implementation starts, add:
5. **Master index / playbook**: navigation and workflow support
6. **Cross-reference / gap log / IDR**: promotion tracking and decision traceability

## Immediate Next Steps

1. **This week**: Complete Git dossier and write Mercurial dossier
2. **Next week**: Write Perforce and Plastic SCM dossiers
3. **Week 3**: Synthesize Track 1 (Continuous Capture)
4. **Week 4**: Synthesize Track 2 (Gate Workflows)
5. **Week 5**: Write Translation Memo 1 (Snap Semantics)

## Key Questions for Convergence

As you research, keep these core questions in mind:

1. **What is the relationship between `snap` and `commit`?**
   - Is a snap buildable? Does it have a message? Who sees it?

2. **How do gates relate to branches?**
   - Are gates like branches? Like CI stages? Something new?

3. **What does it mean to preserve a superposition?**
   - How is it stored? How is it resolved? Can it be reopened?

4. **What requires server authority?**
   - Can you snap offline? What about publish?

5. **How does Convergence compare to Git + GitHub Actions?**
   - What's genuinely new vs. better integrated?

## Resources

### Existing Convergence Docs to Cross-Reference
- `docs/vision/001-convergence-platform-vision.md` — Long-horizon goals
- `docs/architecture/01-concepts-and-object-model.md` — Core definitions
- `docs/architecture/02-repo-gates-lanes-scopes.md` — Gate semantics
- `docs/architecture/04-superpositions-and-resolution.md` — Superposition theory

### External Resources to Monitor
- Hacker News (vcs, git tags)
- Lobste.rs (vcs, git, mercurial tags)
- Git Merge conference (annual)
- sapling-scm.com and martinvonz.github.io/jj/ (emerging systems)

---

**Ready to start?** Begin with completing the Git dossier, then move to Mercurial. The comparison between those two contemporary systems (Git won, but why?) will inform much of Convergence's design philosophy.
