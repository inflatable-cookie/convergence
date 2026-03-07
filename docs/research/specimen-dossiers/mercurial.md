# System Dossier: Mercurial

**Product Identity**: Distributed version control system created by Matt Mackall in 2005, same era as Git.

**Era Context**: Post-BitKeeper, same motivation as Git. Designed for ease of use and cross-platform compatibility. Python implementation.

---

## Defining Architectural Bets

1. **Revlog storage format** — Delta-compressed append-only logs rather than Git's object store
2. **Changeset-based** — Focus on changesets rather than Git's snapshot-based model
3. **Extensible by design** — Extensions for evolve, topics, shelve, etc.
4. **Cross-platform first** — Windows support from day one
5. **UI clarity** — Commands named for what they do (`hg add`, `hg commit`)
6. **Phases** — Changesets have phases (draft/public/secret) controlling mutability

---

## Standout Strengths

- **Ease of use** — Simpler command set than Git, more consistent
- **Extensibility** — Rich extension ecosystem (evolve, topics, crecord, etc.)
- **Revlog format** — Efficient delta storage, fast linear history access
- **Windows support** — Better Windows experience historically
- **Changeset evolution** — `hg evolve` for safe history rewriting
- **Shelve** — Built-in shelving without stash complexity
- **Topics** — Lightweight branching without heavy bookmarks

---

## Chronic Pain Points

1. **Network effects lost to Git** — GitHub and ecosystem dominance
2. **Performance on large repos** — Python slower than Git's C
3. **Branching model confusion** — Named branches vs. bookmarks vs. topics
4. **Monorepo scaling** — Similar challenges to Git
5. **Bitbucket dropped support** — 2020, major blow to ecosystem

---

## Between-Release Evolution

| Era | Change | Rationale |
|-----|--------|-----------|
| Early | Named branches | Branches are permanent records |
| 1.4+ | Bookmarks | Lightweight branching like Git |
| 2.1+ | Phases | Draft/public/secret for mutability |
| 2.3+ | RevlogNG | Improved storage format |
| 3.2+ | Cextensions | Performance improvements |
| 4.4+ | Rust core | Incremental rewrite for speed |
| Ongoing | Changeset evolution | Safe distributed history rewriting |

---

## Technical Deep Dives

### Revlog Format

Mercurial's **revlog** (revision log) is a delta-compressed append-only format:

- **Index file** — Maps revision numbers to offsets
- **Data file** — Compressed deltas or fulltext
- **RevlogNG** — Modern format with better compression

Unlike Git's object store where everything is content-addressed:
- Revlogs are append-only logs of related data
- Manifest revlog tracks file states
- Filelog per file tracks content history
- Changeset revlog links manifests

This makes linear history traversal fast but complicates some operations.

### Phases Model

Mercurial introduced **phases** to manage mutability:

| Phase | Description | Can Rewrite? |
|-------|-------------|--------------|
| `secret` | Local only, never shared | Yes |
| `draft` | Local, will be shared | Yes |
| `public` | Shared with others | No |

Phases propagate on push/pull. This prevents accidental history rewriting of shared commits.

**Convergence consideration**: Phases are a lightweight gate concept. Convergence could learn from this for promotion semantics.

### Extensions Architecture

Mercurial is designed to be extended:

- **Evolve** — Safe history rewriting with obsolescence markers
- **Topics** — Lightweight feature branches
- **Shelve** — Stash-like functionality
- **Crecord** — Interactive chunk selection
- **Largefiles** — Git LFS equivalent

Extensions can modify nearly any behavior, leading to rich ecosystem but also fragmentation.

---

## Convergence-Relevant Lessons

### What to study

1. **Revlog efficiency** — Delta compression patterns for storage
2. **Phases** — Lightweight mutability control precedent
3. **Evolve** — Obsolescence markers for history rewriting
4. **Topics** — Lightweight branch-like workflows

### What to reject early

1. **Extension fragmentation** — Convergence should have cohesive features
2. **Python performance ceiling** — Rust is right choice for Convergence
3. **Multiple branching models** — Confusing; pick one clear model

### What to prototype/compare

1. **Phases vs. gates** — How lightweight can promotion policy be?
2. **Shelve vs. snap** — How do WIP preservation patterns differ?
3. **Evolve for convergence** — Could obsolescence track bundle ancestry?

---

## Source Inventory

| Source | Type | Confidence | Notes |
|--------|------|------------|-------|
| mercurial-scm.org | Official docs | High | Comprehensive guide |
| wiki.mercurial-scm.org | Community docs | Medium | Extension documentation |
| hgbook.red-bean.com | Book | High | Free online |
| selenic.com/repo/hg | Source | High | Python, some C/Rust |
| Mercurial Sprint talks | Conference | Medium | Developer meetups |

---

## Why Git Won (Lessons for Convergence)

1. **GitHub network effect** — Platform beats tooling
2. **Linux halo effect** — Linus's credibility
3. **Performance** — C beat Python for large repos
4. **Marketing** — "Git is fast, distributed, modern"
5. **Timing** — Bitbucket picked Git over Mercurial

**For Convergence**: Technical superiority alone is insufficient. Need clear differentiation and platform story.

---

## Related Convergence Tracks

- Track 1: Continuous capture (shelve vs. stash patterns)
- Track 2: Gate-based workflows (phases as lightweight gates)
- Track 5: Large repository handling (revlog vs. object store)
- Track 7: Workspace state management (shelve, evolve)
