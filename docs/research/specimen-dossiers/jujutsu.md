# System Dossier: Jujutsu (jj)

**Product Identity**: Version control system by Martin von Zweigbergk (Google), started 2019, now full-time Google project.

**Era Context**: Post-Git, asks "what if we kept the good parts but fixed the UX?" Rust implementation, Git-compatible backend.

---

## Defining Architectural Bets

1. **Working copy as commit** — Your working directory is always a commit (the "working commit"). Changes are automatically recorded.
2. **Immutable history by default** — No `git commit --amend` that changes history; instead, new commits with auto-rebase.
3. **Conflicts as first-class** — Conflicts can be committed and resolved later. Multiple conflict representations stored.
4. **Git compatibility** — Can use Git as backend; speaks Git protocol. Migration path from Git.
5. **Operation log** — All operations (including "undo") are logged. Can undo an undo.
6. **Described vs. change-based** — Tracks both commit descriptions and actual changes.

---

## Standout Strengths

- **No staging area confusion** — Changes are automatically in the commit
- **Conflict commit support** — Can run tests on conflicting code, collaborate on resolution
- **Automatic rebase** — Descendants auto-rebase when parent changes
- **Operation undo** — `jj undo` undoes any operation, including previous undos
- **Multiple working commits** — Can have multiple "heads" in one workspace
- **Git interop** — Can clone Git repos, push/pull to Git remotes

---

## Chronic Pain Points

1. **New mental model** — Even experienced Git users need unlearning
2. **Immaturity** — Still pre-1.0, API/tooling ecosystem developing
3. **Performance** — Rust is fast but some operations slower than Git native
4. **Documentation** — Good but not as extensive as Git
5. **Adoption friction** — Teams need to adopt together for full benefit

---

## Between-Release Evolution

Jujutsu is rapidly evolving. Key developments:
- Git backend stabilization
- Operation log refinement
- Conflict representation improvements
- Colocated Git repos (jj and git commands work on same repo)

---

## Convergence-Relevant Lessons

### Directly relevant to Convergence

1. **Working copy as commit** — Similar to Convergence's `snap` concept. Shows UX approach.
2. **Conflict commits** — Precedent for Convergence's `superposition` concept. Shows it's viable.
3. **Operation log** — Convergence could track all operations for audit/undo.

### What to study

- How conflicts are serialized and stored
- How automatic rebase handles descendants
- How Git backend compatibility is maintained
- Operation log storage and query patterns

### What to differentiate

- Jujutsu keeps Git's distributed model; Convergence wants server authority
- Jujutsu focuses on individual developer UX; Convergence focuses on organizational convergence
- Jujutsu has no built-in gate/policy concept

---

## Source Inventory

| Source | Type | Confidence | Notes |
|--------|------|------------|-------|
| martinvonz.github.io/jj/ | Official docs | High | Comprehensive |
| github.com/martinvonz/jj | Source | High | Rust |
| "The Jujutsu Version Control System" talk | Conference | High | Martin von Zweigbergk |
| Various blog posts | Experience | Medium | Migration stories |

---

---

## Technical Deep Dives

### Conflict Representation

Jujutsu treats conflicts as **first-class data** that can be committed, stored, and resolved later:

- **Conflict materialization** — When a merge produces conflicts, they're stored in a structured format
- **Conflict commits** — You can commit with conflicts; the commit records the conflict state
- **Multiple parents** — Conflict commits have multiple parents representing the conflicting sides
- **Conflict markers** — Materialized as files with conflict markers similar to Git, but stored structurally

The conflict format tracks:
- The base version (common ancestor)
- Each conflicting side (parent commits)
- The state of resolution (resolved or not)

This allows:
- **Testing conflicting code** — Run tests before deciding on resolution
- **Collaborative resolution** — Share conflict commits for others to help resolve
- **Deferred resolution** — Commit now, resolve later when more context available
- **Resolution provenance** — Track who resolved and when

**Convergence consideration**: Direct precedent for Convergence's `superposition` concept. Shows that conflict-as-data is viable and useful.

### Operation Log

Jujutsu records **every operation** in a log, enabling sophisticated undo:

```
jj op log          # Show all operations
jj undo            # Undo last operation
jj undo --what op  # Undo specific operation
jj op restore <id> # Restore to any prior state
```

Unlike Git's reflog (which only records ref updates), Jujutsu's operation log records:
- Commit description changes
- Working copy changes
- Branch movements
- Rebases and rewrites

This enables **undo of undo** — you can undo an undo without losing work.

The operation log is stored in a custom format alongside the repo data.

**Convergence consideration**: Convergence could track all operations for audit and provenance. Server authority makes this even more powerful.

### Git Backend Compatibility

Jujutsu can use Git as a backing store while providing its own UX:

- **Colocated repos** — `.git` and `.jj` can coexist; both `git` and `jj` commands work
- **Git mapping** — Jujutsu commits map to Git commits; conflicts stored in Git tree
- **Interoperability** — Can push/pull to Git remotes; Git users see normal history

This provides a **migration path** — teams can adopt Jujutsu gradually without breaking Git workflows.

**Convergence consideration**: Git compatibility is valuable for adoption. Convergence should consider import/export paths from Git.

---

## Related Convergence Tracks

- Track 1: Continuous capture (direct comparison)
- Track 3: Conflict preservation (direct precedent)
- Track 7: Workspace state management
- Track 11: Review workflows (jj has no built-in review)
