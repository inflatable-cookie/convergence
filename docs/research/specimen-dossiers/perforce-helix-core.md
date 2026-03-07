# System Dossier: Perforce Helix Core

**Product Identity**: Centralized version control system created by Christopher Seiwald in 1995. Now Perforce (acquired 2013).

**Era Context**: Pre-distributed VCS era. Designed for enterprise, games, and large binary assets. Commercial with free tier.

---

## Defining Architectural Bets

1. **Centralized model** — Single source of truth server, clients are thin
2. **File-level locking** — Exclusive checkout prevents merge conflicts on unmergeable files
3. **Depot namespace** — Unified view of all code as file paths
4. **Client views** — Each workspace maps depot paths to local paths flexibly
5. **Changelists** — Atomic units of work, can be shelved (pending) or submitted
6. **Streams** — Branching evolved into structured hierarchy with inheritance

---

## Standout Strengths

- **File locking** — Essential for binary assets (art, video, design files)
- **Granular permissions** — Path-based access control at depot/directory level
- **Scale** — Handles petabyte-scale repositories (Google used internally)
- **Game industry standard** — Ubiquitous in AAA game development
- **Changelist workflow** — Logical changes vs. Git's file-based staging
- **Shelving** — Share work-in-progress without submitting
- **Streams** — Structured branching with merge path enforcement

---

## Chronic Pain Points

1. **Centralized fragility** — Server downtime blocks all work
2. **Offline limitation** — Minimal capability without server connection
3. **Branching cost** — Historically expensive; Streams improved but still structured
4. **Learning curve** — Concepts (depot, client, stream) foreign to Git users
5. **Licensing cost** — Commercial model limits adoption outside enterprise
6. **Merge experience** — Perforce's 3-way merge historically weaker than Git
7. **DVCS integration** — Git Fusion/Helix4Git are bolt-ons, not native

---

## Between-Release Evolution

| Era | Change | Rationale |
|-----|--------|-----------|
| 1995 | P4 release | Centralized VCS for enterprise |
| 2005+ | DVCS pressure | Git/Mercurial growth |
| 2011+ | Streams | Structured branching hierarchy |
| 2012+ | Git Fusion | Git compatibility layer |
| 2015+ | Helix branding | Rebrand with scale messaging |
| 2018+ | Helix4Git | Native Git support alongside P4 |
| 2020+ | SaaS offering | Helix Core Cloud for convenience |

---

## Technical Deep Dives

### Depot and Client View Model

Perforce organizes everything in **depots** with a filesystem-like namespace:

```
//depot/main/src/...
//depot/release/1.0/...
//depot/assets/characters/...
```

**Client views** map depot paths to workspace paths:

```
View:
    //depot/main/src/... //client/src/...
    //depot/assets/...    //client/assets/...
    -//depot/assets/temp/... //client/assets/temp/...
```

Views can:
- Include/exclude paths
- Remap directory structures
- Use wildcards for batch mapping

This allows flexible workspace composition without monolithic checkouts.

### Changelist Model

Work happens in **changelists** (numbered change tickets):

1. **Open files** — Mark files for edit/add/delete in a changelist
2. **Work** — Edit files locally
3. **Reconcile** — Compare workspace to depot
4. **Shelve** (optional) — Store pending work on server without submitting
5. **Submit** — Atomic commit to depot

Shelving enables:
- Backup of WIP
- Code review before submit
- Handoff between developers
- CI testing of pending changes

### Streams

**Streams** (introduced 2011) structure branching:

```
mainline
├── development
│   ├── feature-1
│   └── feature-2
└── release/1.0
    └── release/1.0-patch
```

Each stream has:
- **Type**: mainline, development, release, task, virtual
- **Parent**: inheritance path
- **Paths**: what flows up/down
- **Options**: merge policy, locked/unlocked

Merge paths are **enforced** — you can't merge from arbitrary streams.

**Convergence consideration**: Direct precedent for gate graphs and promotion policy. Shows that structured promotion paths are valuable at scale.

### File Locking

**Exclusive locking** (exclusive checkout):

```
p4 edit -l file.bin    # Lock for exclusive access
```

Only one user can lock a file at a time. Critical for:
- Binary assets (can't merge)
- Exclusive resources
- Design files
- Video/audio assets

**Convergence consideration**: File locking is essential for game/VFX workflows. Convergence needs equivalent capability.

---

## Convergence-Relevant Lessons

### What to study

1. **Streams as gate graphs** — Structured promotion paths with policy
2. **Changelist workflow** — Logical changes as units of work
3. **File locking** — Required for unmergeable assets
4. **Shelving** — Server-side WIP preservation
5. **Path-based permissions** — Granular access control

### What to reject early

1. **Full centralization** — Convergence wants offline capability
2. **Server fragility** — Must be resilient to connectivity issues
3. **Expensive branching** — Convergence wants lightweight scopes
4. **Commercial licensing** — Open model preferred

### What to prototype/compare

1. **Stream promotion vs. Convergence gates** — How rigid/flexible should promotion be?
2. **Shelve vs. snap** — Server-side WIP vs. local capture
3. **Locking integration** — How to handle unmergeable files in distributed model?
4. **Changelist vs. bundle** — Similar concepts, different implementations

---

## Source Inventory

| Source | Type | Confidence | Notes |
|--------|------|------------|-------|
| helixdocs.perforce.com | Official docs | High | Comprehensive |
| Perforce blog | Product news | Medium | Marketing-heavy |
| GDC Vault talks | Conference | Medium | Game industry usage |
| "Practical Perforce" | Book | High | Laura Wingerd, 2005 |
| Various studio postmortems | Experience | Medium | Production patterns |

---

## Game Industry Usage Patterns

Perforce dominates game development because:

1. **Binary assets** — Art, audio, video need locking
2. **Large repos** — Multi-terabyte repositories common
3. **Non-technical users** — Artists/designers need simple workflows
4. **Shelving for review** — Asset review before submit
5. **Integrate patterns** — Merge down, copy up between streams

**Convergence implication**: If targeting games, must handle binary assets and locking.

---

## Related Convergence Tracks

- Track 2: Gate-based workflows (Streams are direct precedent)
- Track 3: Conflict preservation (locking prevents conflicts)
- Track 4: Bundle semantics (changelists vs. bundles)
- Track 6: Large binary workflows (locking essential)
- Track 8: Permissions and access control (path-based perms)
- Track 9: Server authority models (centralized vs. distributed)
