# Convergence Translation Memos

Purpose: convert comparative research into Convergence-facing recommendations without prematurely freezing them into implementation contracts.

## When to write a memo

Write a translation memo when a research thread can answer:
- The Convergence problem statement
- The external evidence behind the recommendation
- The tradeoffs Convergence would inherit
- The prototype, metric, or contract work needed before adoption

## Memo Outcomes

Each memo should end with one of:
- `promote to concept work` — Ready to inform architecture or roadmap
- `prototype first` — Needs experimental validation before commitment
- `watch only` — Interesting but not yet actionable
- `reject for Convergence` — Explicitly not aligned with project goals

## Memo Template

Use `docs/research/templates/translation-memo-template.md`.

Memos follow this structure:
1. Problem Statement — What Convergence problem this addresses
2. External Evidence — What systems were studied and what they do
3. Cross-System Comparison — How approaches differ
4. Convergence Implications — Specific recommendations
5. Tradeoffs Accepted — What we give up for this choice
6. Open Questions — What remains uncertain
7. Recommended Next Step — Promote, prototype, watch, or reject

## Current Memos

### Phase 3 Complete (g01.045)

- [001-snap-semantics.md](./001-snap-semantics.md) — Outcome: `prototype first`
- [002-gate-policy-model.md](./002-gate-policy-model.md) — Outcome: `prototype first`
- [003-superposition-as-data.md](./003-superposition-as-data.md) — Outcome: `promote to concept work`

### Summary

| Memo | Concept | Outcome | Key Recommendation |
|------|---------|---------|-------------------|
| **001** | `snap` | Prototype first | Automatic capture, optional message |
| **002** | `gate` | Prototype first | Server-authoritative, configurable policy |
| **003** | `superposition` | Promote to concept work | First-class conflict with full provenance |

### Next Steps

Based on memo outcomes:

1. **Prototype snaps** — Build and test automatic capture UX
2. **Prototype gates** — Build linear gate chain with simple policy
3. **Concept work: superpositions** — Update architecture docs with detailed structure

## Future Memos (Proposed)

4. **Server authority balance** — What's centralized vs. distributed
   - Compare: Git (distributed with upstream), Perforce (centralized), Radicle (p2p)
   - Question: Can you snap offline? What's the minimal server?

5. **Large binary handling** — How to handle game/VFX assets
   - Compare: Git LFS, Perforce locking, Plastic semantic diff
   - Question: How does Convergence handle unmergeable files?

## Research Program Status

The core research program (Phases 1-3) is complete. Three memos produced with actionable outcomes. Prototype and concept work should proceed based on memo recommendations.
