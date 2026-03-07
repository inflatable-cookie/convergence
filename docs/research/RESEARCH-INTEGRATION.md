# Research Integration Summary

**Status**: Phases 1-3 Complete — Architecture Updated — Prototypes Ready
**Date**: 2026-03-07

## Overview

The Comparative Research Program (roadmaps g01.043-g01.045) has completed its core phases, producing evidence-based design foundations for Convergence's novel concepts.

The implementation bridge is now bootstrapped as well:
- [master-index.md](./master-index.md) — navigation from architecture and implementation concerns to research
- [research-to-implementation-playbook.md](./research-to-implementation-playbook.md) — research-aware delivery workflow
- [quick-start-checklist.md](./quick-start-checklist.md) — short checklist for contributors
- [research-to-architecture-crossref.md](./research-to-architecture-crossref.md) — memo promotion and architecture gap tracking
- [gaps-found-during-implementation.md](./gaps-found-during-implementation.md) — implementation-discovered research gaps
- [templates/implementation-decision-record.md](./templates/implementation-decision-record.md) — durable traceability for implementation decisions

## Deliverables Completed

### Phase 1: Specimen Dossiers (g01.043) ✅

Five system dossiers documenting architectural approaches:

| System | Model | Key Insight for Convergence |
|--------|-------|---------------------------|
| [Git](specimen-dossiers/git.md) | Distributed + explicit | Object store design, staging complexity |
| [Mercurial](specimen-dossiers/mercurial.md) | Distributed + phases | Phases as lightweight mutability control |
| [Perforce](specimen-dossiers/perforce-helix-core.md) | Centralized + streams | Stream promotion as gate precedent |
| [Plastic](specimen-dossiers/plastic-scm.md) | Hybrid + semantic | Visual branching, semantic merge |
| [Jujutsu](specimen-dossiers/jujutsu.md) | Git-backed + automatic | Working copy as commit, conflicts-as-data |

### Phase 2: Value Tracks (g01.044) ✅

Three syntheses of cross-system patterns:

| Track | Core Question | Key Finding |
|-------|---------------|-------------|
| [Track 1](value-tracks/continuous-capture-vs-explicit-commit.md) | What is a `snap`? | Automatic capture, optional message (Jujutsu precedent) |
| [Track 2](value-tracks/gate-based-workflows.md) | What is a `gate`? | Server-authoritative, configurable policy (Perforce precedent) |
| [Track 3](value-tracks/conflict-preservation.md) | What is a `superposition`? | First-class conflict with provenance (Jujutsu/Pijul precedent) |

### Phase 3: Translation Memos (g01.045) ✅

Three memos with actionable outcomes:

| Memo | Concept | Outcome | Key Recommendation |
|------|---------|---------|-------------------|
| [001](translation-memos/001-snap-semantics.md) | `snap` | **Prototype first** | Automatic capture, optional message |
| [002](translation-memos/002-gate-policy-model.md) | `gate` | **Prototype first** | Server-authoritative, configurable policy |
| [003](translation-memos/003-superposition-as-data.md) | `superposition` | **Promote to concept work** | First-class conflict with full provenance |

## Architecture Updates

Based on research findings, the following architecture documents have been updated:

### [01-concepts-and-object-model.md](../architecture/01-concepts-and-object-model.md)

Updated with:
- Snap: automatic capture, optional message, build status tracking
- Gate: server-authoritative, configurable policy, explicit promotion
- Superposition: resolution recording, reopenable resolutions
- Research integration section

### [04-superpositions-and-resolution.md](../architecture/04-superpositions-and-resolution.md)

Major update with:
- Detailed superposition structure (ID, variants, resolution, status)
- Full provenance tracking (who, when, how, why)
- Reopenable resolutions
- Resolution methods (TakeA/B/N, MergeManual, Automated, ThirdWay)
- Research-informed design decisions

## Prototype Specifications

Two prototype specs created based on memo recommendations:

### [Prototype: Automatic Snap Capture](../architecture/prototype-snap-capture.md)

**Goal**: Validate automatic snap capture UX

**Key Design Decisions**:
- Hybrid trigger (time-based + change-based)
- Optional message (can add later)
- Build status tracking as metadata
- Storage optimization via content-addressing

**Success Criteria**:
- No lost work
- Transparent capture
- User understands snap vs. publish
- Storage overhead < 20%

### [Prototype: Linear Gate Chain](../architecture/prototype-gate-chain.md)

**Goal**: Validate gate policy and promotion flow

**Key Design Decisions**:
- Three-gate chain: Dev → Integration → Release
- Server-authoritative policy
- Explicit promotion with policy checking
- Approval tracking

**Success Criteria**:
- Clear promotion path
- Policy enforceable
- Visible status
- Attributable actions

## Implementation Roadmap

Based on research outcomes, recommended implementation order:

### Immediate (Next 2-4 weeks)

1. **Superposition architecture** — Update implementation to match detailed spec
   - Add superposition ID to manifest entries
   - Implement resolution recording
   - Add provenance tracking
   - Status: Ready to implement (detailed in architecture doc)

2. **Begin snap prototype** — Start automatic capture implementation
   - Time-based capture daemon
   - Basic snap creation
   - Storage metrics
   - Status: Spec complete, ready to code

### Short Term (1-2 months)

3. **Complete snap prototype** — Finish and test
   - Change-based capture
   - Message attachment
   - Build status tracking
   - User study
   - Status: Depends on prototype phase 1

4. **Begin gate prototype** — Start gate chain implementation
   - Gate configuration
   - Bundle creation
   - Basic promotion
   - Status: Spec complete, ready after snap prototype

### Medium Term (2-3 months)

5. **Complete gate prototype** — Finish and test
   - Policy checking
   - Approval workflow
   - Gate visualization
   - User study
   - Status: Depends on gate prototype phase 1

6. **Integration** — Combine snap + gate prototypes
   - Publish from snap to gate
   - End-to-end workflow
   - Status: Depends on both prototypes

## Using Research During Delivery

For active implementation work:
1. Start with [master-index.md](./master-index.md)
2. Read the relevant translation memo
3. Check [research-to-architecture-crossref.md](./research-to-architecture-crossref.md) for current alignment and prototype-gated gaps
4. Follow [research-to-implementation-playbook.md](./research-to-implementation-playbook.md)
5. Record missing research in [gaps-found-during-implementation.md](./gaps-found-during-implementation.md)

## Design Decisions Validated by Research

### Snap: Automatic Capture

**Decision**: Snaps are captured automatically, not explicitly

**Research Basis**: 
- Git's explicit commit loses work when users forget
- Jujutsu proves automatic capture is viable
- Editor auto-save shows user acceptance

**Tradeoff**: More history volume, but no lost work

### Gate: Server-Authoritative Policy

**Decision**: Gate policy lives on and is enforced by server

**Research Basis**:
- Perforce streams show value of structured promotion
- GitHub branch protection shows user expectation of server enforcement
- Distributed policy (Git hooks) is fragile

**Tradeoff**: Requires connectivity, but policy is enforceable

### Superposition: First-Class Data

**Decision**: Conflicts are preserved as data with full provenance

**Research Basis**:
- Jujutsu proves conflict commits work
- Pijul shows formal conflict theory
- Git's immediate resolution loses work

**Tradeoff**: More complexity, but deferred resolution and collaboration

## Comparison to Existing Systems

### Convergence vs. Git

| Aspect | Git | Convergence |
|--------|-----|-------------|
| Capture | Explicit commit | Automatic snap |
| History | Immutable commits | Snap history + bundle promotion |
| Conflicts | Block commit | Preserve as superposition |
| Promotion | Merge to branch | Promote through gates |

### Convergence vs. Perforce

| Aspect | Perforce | Convergence |
|--------|----------|-------------|
| Model | Centralized | Server-authoritative but offline-capable |
| Branches | Streams (hierarchical) | Gates (configurable DAG) |
| Locking | File locking | Planning for gate-level policy |
| Capture | Explicit changelist | Automatic snap |

### Convergence vs. Jujutsu

| Aspect | Jujutsu | Convergence |
|--------|---------|-------------|
| Capture | Working copy as commit | Automatic snap |
| Conflicts | Conflict commits | Superpositions with provenance |
| Model | Distributed | Server-authoritative |
| Gates | None | Core feature |

## Open Questions for Prototypes

### Snap Prototype

1. What capture frequency feels right? (5 min? 2 min?)
2. Do users add messages retroactively?
3. How much storage overhead is acceptable?
4. Should we support "squash" before publish?

### Gate Prototype

1. Is linear chain sufficient or do we need DAG?
2. What policy language is intuitive?
3. How many approvals feel right per gate?
4. Is explicit promotion smooth or cumbersome?

## Research Gaps (Future Work)

Not covered in core research, potential Phase 4 (g01.046):

1. **Large binary workflows** — Game/VFX asset management
2. **Server authority models** — Detailed comparison of centralized vs. distributed
3. **Review workflows** — Phabricator, Gerrit, stacked PRs
4. **Identity and provenance** — Sigstore, attestation frameworks
5. **CI/CD integration** — Webhook patterns, required checks

## Conclusion

The research program has provided evidence-based foundations for Convergence's core semantics:

- **Snap**: Automatic, lightweight capture (validated by Jujutsu)
- **Gate**: Structured, policy-enforced promotion (validated by Perforce)
- **Superposition**: First-class conflict with provenance (validated by Jujutsu/Pijul)

Architecture documents have been updated with research findings. Prototype specifications are complete and ready for implementation.

**Next Action**: Begin implementation of superposition architecture updates and snap prototype.

## References

- [Research README](README.md) — Program overview
- [Getting Started](GETTING-STARTED.md) — Execution guide
- [Roadmap g01.043](../roadmaps/g01/043-research-baseline-specimen-dossiers.md)
- [Roadmap g01.044](../roadmaps/g01/044-research-core-value-track-syntheses.md)
- [Roadmap g01.045](../roadmaps/g01/045-research-translation-memos-and-architecture-input.md)
