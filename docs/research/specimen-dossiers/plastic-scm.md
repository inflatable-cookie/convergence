# System Dossier: Plastic SCM (Unity Version Control)

**Product Identity**: Version control system created by Codice Software in 2005, acquired by Unity in 2020. Now "Unity Version Control."

**Era Context**: Post-Perforce, designed to fix Perforce's pain points while keeping strengths. Targeted game developers.

---

## Defining Architectural Bets

1. **Distributed + Centralized hybrid** — Can work centralized like Perforce or distributed like Git
2. **Semantic merge** — Understand code structure for better merging
3. **Visual branching** — Branch explorer GUI as primary interface
4. **Binary asset handling** — First-class support for game assets
5. **Unity integration** — Deep editor integration
6. **Changeset+branch model** — Branches are first-class, changesets atomic

---

## Standout Strengths

- **Semantic merge** — Diff/merge understands C#, C++, Java structure
- **Visual Branch Explorer** — GUI shows branch topology clearly
- **Unity integration** — In-editor VCS operations
- **Xlinks** — Sub-repository linking with configurable update rules
- **Both modes** — Centralized (lock-friendly) or distributed (offline-friendly)
- **Shelving** — Cloud-based shelved changes
- **Merge-to** — Merge and keep branch active (vs. merge-and-delete)

---

## Chronic Pain Points

1. **Unity association** — Seen as Unity-only tool despite standalone capability
2. **Market share** — Smaller ecosystem than Git or Perforce
3. **Cloud dependency** — Some features require Unity services
4. **Learning curve** — Distinct concepts from both Git and Perforce
5. **Pricing** — Commercial model (free for small teams)

---

## Between-Release Evolution

| Era | Change | Rationale |
|-----|--------|-----------|
| 2005 | Plastic SCM release | Perforce alternative |
| 2010+ | Semantic merge | Differentiator for code merges |
| 2015+ | Cloud expansion | SaaS offering |
| 2020 | Unity acquisition | Strategic platform integration |
| 2021+ | Unity Version Control rebrand | Unified branding |
| 2022+ | DevOps features | CI/CD integration |

---

## Technical Deep Dives

### Semantic Merge

**Semantic merge** analyzes code structure rather than text lines:

- **Language-aware** — Understands C#, C++, Java, Python
- **Method-level diff** — "Method Foo moved and modified" vs. line changes
- **Refactoring detection** — Recognizes moves, renames, extracts
- **Better merge decisions** — Reduces false conflicts

Example: If two developers modify different methods in a file:
- Text merge: potential conflict on overlapping lines
- Semantic merge: clean merge (different methods)

**Convergence consideration**: Semantic understanding is valuable but complex. Convergence may want structural diffs for specific file types.

### Visual Branch Explorer

The **Branch Explorer** is Plastic's signature GUI:

- **Topological view** — Branches as lanes, changesets as nodes
- **Color coding** — Green (merged), red (conflict), yellow (pending)
- **Drag-and-drop merge** — Visual merge initiation
- **Task branches** — Lightweight branches for single tasks
- **Branch metrics** — Age, activity, merge status

This makes branch topology discoverable without command-line archaeology.

**Convergence consideration**: Convergence TUI should make gate/bundle topology similarly discoverable.

### Xlinks (Sub-repositories)

**Xlinks** link external repositories with rules:

```
Xlink: /assets/characters
    Target: /characters_repo/main
    Update rule: Last changeset
```

Update rules:
- **Last changeset** — Always use latest
- **Fixed changeset** — Pin to specific version
- **Branch head** — Follow branch tip

Xlinks are more flexible than Git submodules (which track specific commits).

**Convergence consideration**: Scope/lane dependencies could learn from Xlink rules.

### Distributed vs. Centralized Modes

Plastic can operate in **two modes**:

**Distributed mode**:
- Full local history
- Push/pull between repos
- Work offline
- Like Git

**Centralized mode**:
- Checkin/checkout from central server
- File locking support
- Workspaces track server state
- Like Perforce

Teams can mix modes — some developers centralized, others distributed.

**Convergence consideration**: Hybrid model is interesting. Convergence's server authority could support both local-first and server-first workflows.

---

## Convergence-Relevant Lessons

### What to study

1. **Semantic merge** — Structural diff/merge for reducing conflicts
2. **Visual branch explorer** — GUI patterns for topology visualization
3. **Xlinks** — Flexible sub-repository linking
4. **Hybrid mode** — Supporting both centralized and distributed workflows
5. **Task branches** — Lightweight branches for small units of work

### What to reject early

1. **Unity-specific branding** — Convergence must be platform-agnostic
2. **Commercial licensing** — Prefer open model
3. **Cloud dependency** — Must work air-gapped

### What to prototype/compare

1. **Semantic diff for bundle review** — Structure-aware diffs
2. **Visual gate graph** — Branch Explorer pattern for gates
3. **Scope dependencies** — Xlink-like rules between scopes
4. **Locking in distributed model** — Hybrid approach to file locking

---

## Source Inventory

| Source | Type | Confidence | Notes |
|--------|------|------------|-------|
| plasticscm.com/docs | Official docs | High | Comprehensive |
| Unity documentation | Product docs | High | Post-acquisition |
| Codice blog | Engineering | Medium | Pre-acquisition |
| Game Developer articles | Experience | Medium | User perspectives |
| "Semantic Merge" papers | Research | High | Underlying tech |

---

## Unity Integration Patterns

Plastic's Unity integration demonstrates deep editor VCS integration:

- **Project view badges** — File status in Unity's project view
- **Inspector integration** — VCS actions on selected assets
- **Smart lock** — Automatic locking of scene files on edit
- **Conflict resolution UI** — In-editor merge tool
- **Changeset review** — Browse changesets within editor

**Convergence consideration**: IDE/editor integration should be first-class, not bolt-on.

---

## Related Convergence Tracks

- Track 3: Conflict preservation (semantic merge reduces conflicts)
- Track 4: Bundle semantics (changeset and branch model)
- Track 6: Large binary workflows (Unity asset handling)
- Track 10: CI/CD integration (DevOps features)
- Track 11: Editor integration patterns (Unity integration)
