# 045 - Research Translation Memos and Architecture Input

Status: Complete
Owner: Research
Created: 2026-03-07
Depends on: 044
Vision tags: `RESEARCH`, `ARCH`, `DOCS`

## Target Envelope

| Target | Envelope | Outcome Expectation |
| --- | --- | --- |
| `RESEARCH` actionable output | 3 translation memos with clear outcomes | Research converts to Convergence recommendations |
| `ARCH` evidence-based decisions | Architecture docs updated with research citations | Design rationale references external evidence |
| `DOCS` design rationale | Research-backed documentation of why Convergence differs | Future contributors understand design choices |

## Goal

Convert comparative research into actionable Convergence recommendations. Produce translation memos that either promote findings to concept work, recommend prototypes, or explicitly reject approaches. Feed validated findings into architecture documentation.

## Why This Phase Exists

Research without translation is just accumulation. This phase produces the bridge from "what exists" to "what Convergence should do." Each memo ends with a clear outcome: promote, prototype, watch, or reject.

## Scope

### In Scope

Write three translation memos based on Phase 2 value tracks:

**Memo 1: Snap Semantics**
- Define what a `snap` is and isn't
- Recommend UX for continuous capture
- Specify relationship to `publish`
- Outcome: promote to concept work OR prototype first

**Memo 2: Gate Policy Model**
- Recommend where policy lives (server-side vs. client-side)
- Define promotability checking semantics
- Suggest policy configuration approach
- Outcome: promote to architecture OR prototype first

**Memo 3: Superposition as Data**
- Recommend serialization format for conflicts
- Define resolution semantics and provenance
- Suggest collaboration patterns
- Outcome: promote to architecture OR reject for simplicity

**Architecture Integration**
- Update `docs/architecture/01-concepts-and-object-model.md` with research citations
- Update `docs/architecture/04-superpositions-and-resolution.md` with conflict research
- Document design rationale: why Convergence differs from Git

### Out of Scope

- Prototype implementation (follows from "prototype first" outcomes)
- Additional system dossiers (defer to future research phases)
- Additional value tracks (defer to future research phases)

## Architecture Notes

Translation memos should follow the structure in `docs/research/translation-memos/README.md`:
1. Problem Statement
2. External Evidence
3. Cross-System Comparison
4. Convergence Implications
5. Tradeoffs Accepted
6. Open Questions
7. Recommended Next Step

## Tasks

### A) Translation Memo 1: Snap Semantics

- [ ] Define snap vs. commit distinction
- [ ] Recommend capture triggers (explicit, continuous, hybrid)
- [ ] Specify snap metadata (message optional? buildable?)
- [ ] Document UX recommendations
- [ ] Identify open questions requiring prototype
- [ ] Write outcome: promote/prototype/watch/reject

### B) Translation Memo 2: Gate Policy Model

- [ ] Recommend gate enforcement location
- [ ] Define promotability check semantics
- [ ] Document policy language options (declarative, code, hybrid)
- [ ] Specify integration with CI/CD
- [ ] Identify open questions
- [ ] Write outcome: promote/prototype/watch/reject

### C) Translation Memo 3: Superposition as Data

- [ ] Recommend conflict serialization approach
- [ ] Define resolution semantics
- [ ] Document provenance tracking
- [ ] Specify collaboration on conflicts
- [ ] Identify open questions
- [ ] Write outcome: promote/prototype/watch/reject

### D) Architecture Documentation Updates

- [ ] Update `01-concepts-and-object-model.md` with snap research citations
- [ ] Update `04-superpositions-and-resolution.md` with conflict research
- [ ] Add "Design Rationale" sections referencing research
- [ ] Document why Convergence differs from Git model

### E) Research Program Summary

- [ ] Update `docs/research/README.md` with phase completion
- [ ] Document remaining research gaps
- [ ] Recommend future research phases
- [ ] Archive or update `docs/research/GETTING-STARTED.md`

## Exit Criteria

- 3 translation memos in `docs/research/translation-memos/`
- Each memo has clear outcome (promote/prototype/watch/reject)
- Architecture docs updated with research citations
- Research program documents current state

## Follow-on Roadmaps (Proposed)

If memos recommend "prototype first":
- Prototype: Continuous snap capture UX
- Prototype: Gate policy enforcement
- Prototype: Conflict preservation format

If additional research needed:
- Roadmap 046: Research Large Binary Workflows
- Roadmap 047: Research Server Authority Models
- Roadmap 048: Research Review Workflow Patterns

## References

- `docs/research/translation-memos/README.md` — Memo structure
- `docs/research/GETTING-STARTED.md` — Phase 3 guidance
- `docs/architecture/01-concepts-and-object-model.md`
- `docs/architecture/02-repo-gates-lanes-scopes.md`
- `docs/architecture/04-superpositions-and-resolution.md`
