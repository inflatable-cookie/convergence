# Convergence Comparative Research

Purpose: give Convergence a durable place to study existing version control systems, collaboration platforms, and distributed systems research without mixing raw research into concept contracts or execution roadmaps.

## Why this exists

Convergence aims to solve problems that Git treats as edge cases: large binary churn, unresolved integration state, gated organizational convergence, and provenance across promotion steps. Before inventing solutions, we need to understand:
- Which systems have attempted similar problems
- What architectural bets made them succeed or fail
- What chronic problems appeared at scale
- What Convergence should adopt, avoid, or prototype

Use this research to answer:
- Which VCS or collaboration systems are worth studying for a given problem
- What patterns exist for gate-based workflows, superpositions, and phased convergence
- What failed in previous attempts at "better than Git"
- What should be prototyped before committing to architecture

## Structure

- `master-index.md`: navigate from architecture or implementation concerns to relevant research
- `research-to-implementation-playbook.md`: workflow for using research during delivery
- `quick-start-checklist.md`: short daily checklist for research-aware implementation
- `research-to-architecture-crossref.md`: track how memo findings map into architecture
- `gaps-found-during-implementation.md`: capture missing research discovered while building
- `specimen-dossiers/`: per-system specimen files (Git, Mercurial, Perforce, Plastic SCM, Fossil, Pijul, etc.)
- `value-tracks/`: cross-system syntheses by problem area
- `source-hubs/`: curated source maps and source-quality hierarchy
- `translation-memos/`: Convergence-facing recommendations derived from research
- `templates/`: reusable templates for implementation-traceable research workflows
- `discovery-intake.md`: policy for secondary-channel triage and intake rules
- `discovery-triage-log.md`: staging area for signals from secondary channels

## Operating Model

1. **Start with a problem, not a fandom list.**
2. **Gather primary sources before secondary commentary.**
3. **Record strengths, chronic failures, and between-release corrections together.**
4. **Convert findings into Convergence implications only after cross-system comparison.**
5. **Promote stable conclusions into `docs/architecture/` or `docs/roadmaps/` only when the recommendation is specific enough to constrain design or execution.**

## Source Hierarchy

Prefer sources in this order:
1. Official docs, release notes, source trees, design documents, and postmortems
2. Academic papers on version control, distributed systems, and collaboration
3. Conference talks (Git Merge, academic workshops), engineering blogs with specific technical claims
4. Community synthesis only when it points back to stronger sources

## Research Outputs

Every meaningful research batch should leave at least one durable artifact:
- A system dossier update
- A value-track synthesis
- A source-hub update
- A Convergence translation memo

## Using This Research During Delivery

When research starts actively shaping implementation work:
1. Check `master-index.md` to find the relevant memo, value track, dossier, and prototype.
2. Use `research-to-implementation-playbook.md` for the expected discovery -> decision -> implementation -> review loop.
3. Use `research-to-architecture-crossref.md` to see which memo findings are already aligned in architecture and which are still prototype-gated or missing.
4. Record missing research in `gaps-found-during-implementation.md` instead of leaving it in review comments or local notes.
5. Use `templates/implementation-decision-record.md` when an implementation choice needs durable research traceability.

## Promotion Rule

Keep tentative findings here until they can answer all of:
- What problem Convergence is solving
- Which evidence supports the recommendation
- Which tradeoffs Convergence accepts
- What must be measured or prototyped before adoption

## Convergence-Specific Research Priorities

Based on the core concepts (`snap`, `publish`, `bundle`, `promote`, `release`, `superposition`):

1. **Continuous capture vs. explicit commit** — How do systems handle work-in-progress state?
2. **Gate-based workflows** — What patterns exist for phased convergence and policy enforcement?
3. **Conflict preservation** — Which systems treat conflicts as first-class data?
4. **Large binary handling** — How do game/VFX studios manage asset workflows?
5. **Server authority models** — What are the tradeoffs in centralized vs. federated vs. peer-to-peer?
6. **Provenance and audit** — How do systems track "who did what when" across promotion steps?
7. **Workspace state vs. commit graph** — Systems that decouple capture from publication

## Roadmap Integration

The research program is tracked as formal Convergence roadmaps:

| Roadmap | Phase | Status |
|---------|-------|--------|
| **g01.043** | Baseline Specimen Dossiers | ✅ Complete |
| **g01.044** | Core Value Track Syntheses | ✅ Complete |
| **g01.045** | Translation Memos and Architecture Input | ✅ Complete |
| **g01.046** | Research Expansion (optional) | 📋 Proposed |

See individual roadmap files for execution details.

## Phase 1 Results

Five specimen dossiers completed covering different architectural approaches:

- **Git** — Distributed, object store, explicit staging
- **Mercurial** — Distributed, revlog, phases for mutability
- **Perforce Helix Core** — Centralized, streams as gates, file locking
- **Plastic SCM** — Hybrid, semantic merge, visual branching
- **Jujutsu** — Distributed (Git-backed), conflicts-as-data, operation log

## Phase 2 Results

Three core value tracks synthesized:

1. **Track 1: Continuous Capture vs. Explicit Commit** — What is a `snap`?
2. **Track 2: Gate-Based Workflows** — How do gates and promotion work?
3. **Track 3: Conflict Preservation** — How do `superpositions` work as data?

## Phase 3 Results

Three translation memos produced:

| Memo | Concept | Outcome |
|------|---------|---------|
| 001 | Snap semantics | `prototype first` |
| 002 | Gate policy model | `prototype first` |
| 003 | Superposition as data | `promote to concept work` |

## Research Program Complete

The core research program (Phases 1-3) is complete. **5 specimen dossiers**, **3 value tracks**, and **3 translation memos** provide evidence-based foundation for Convergence design.

## Implementation Bridge Status

The implementation bridge is now bootstrapped:
- `master-index.md` maps architecture and prototype questions to research artifacts
- `research-to-implementation-playbook.md` defines the expected workflow for research-aware delivery
- `research-to-architecture-crossref.md` tracks memo promotion and remaining gaps
- `gaps-found-during-implementation.md` is ready to capture missing research discovered while building
- `templates/implementation-decision-record.md` gives implementation work a durable research traceability record

## Next Steps

Based on research outcomes:

1. **Prototype snaps** — Build automatic capture UX
2. **Prototype gates** — Build linear gate chain
3. **Concept work: superpositions** — Update architecture docs
4. **Optional: Phase 4** — Additional tracks if needed (g01.046)

See individual roadmap files for detailed execution guidance.
