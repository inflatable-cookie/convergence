# Track 3: Conflict Preservation and Superpositions

## Problem Statement

Most VCS treat merge conflicts as temporary failures to resolve immediately:

- **Git** — Conflicts are working directory state; must resolve before commit
- **Mercurial** — Similar; conflicts block commit
- **SVN** — Conflicts block commit with `.mine`/`.rN` files

What if conflicts were **first-class data** that could be:
- Committed and shared?
- Resolved later?
- Collaborated on?
- Reopened?

Convergence introduces `superposition` as a conflict object. What prior art exists?

## Cross-System Comparison

### Git: Conflicts as Working Directory State

Git treats conflicts as **local working directory state**:

```
# Merge produces conflict markers
$ git merge feature
Auto-merging file.txt
CONFLICT (content): Merge conflict in file.txt
Automatic merge failed; fix conflicts and commit

# File contains markers
<<<<<<< HEAD
console.log("main");
=======
console.log("feature");
>>>>>>> feature
```

**Resolution required before commit**:
- Cannot commit with conflicts (without `-m "message"` and special handling)
- Conflicts are not recorded in history
- Resolution is lost if abandoned

**Partial mitigations**:
- `git merge --abort` — Abandon merge, restore state
- `git rerere` — Remember resolutions (opt-in, limited)

**Strengths**:
- Forces resolution (conflicts don't persist)
- Simple model (resolve or abort)

**Pain Points**:
- Cannot share conflicts for collaborative resolution
- Cannot defer resolution
- Cannot test conflicting state
- Rerere is opt-in and repo-local

### Git: Rerere (Reuse Recorded Resolution)

Git's `rerere` ("reuse recorded resolution"):

```bash
git config --global rerere.enabled true

# Record resolution automatically
# Reuse same resolution on same conflict
```

**Limitations**:
- Opt-in (most users don't know about it)
- Local only (not shared)
- Heuristic matching (may apply wrong resolution)
- No conflict provenance

### Pijul: Patch-Based Conflicts

Pijul uses **patch theory** and treats conflicts as **algebraic objects**:

- **Patches are commutative** — Order doesn't matter
- **Conflicts are explicit** — Represented as conflicting patches
- **Conflict resolution is a patch** — Recorded separately

**Conflict representation**:
```
# Pseudocode representation
conflict: {
  patch_a: "add line X",
  patch_b: "add line Y at same location",
  resolution: null  # or "take X", "take Y", "take both", custom
}
```

**Strengths**:
- Conflicts are first-class
- Can have multiple conflicting patches
- Resolution is reversible

**Pain Points**:
- New mental model (patches vs. snapshots)
- Smaller ecosystem
- Performance challenges

### Jujutsu: Conflict Commits

Jujutsu allows **commits with conflicts**:

```bash
# Merge produces conflict commit
$ jj merge feature
Created conflict commit: kmtnuoml 2f0d1b3e

# Can see conflict in log
$ jj log
@  kmtnuoml 2f0d1b3e  (conflict) Merge feature into main

# Can resolve later
$ jj resolve
```

**Conflict representation**:
- Stored in commit tree with special markers
- Multiple parents represent conflicting sides
- Can materialize with conflict markers or other formats

**What you can do with conflict commits**:
- **Commit with conflicts** — Record the conflict state
- **Test conflicting code** — Run tests on conflict state
- **Share conflicts** — Push conflict commit for help
- **Resolve later** — No immediate pressure
- **Collaborative resolution** — Others can resolve and update

**Strengths**:
- Conflicts are persistent and shareable
- No lost work from abandoned merges
- Can reason about conflicts

**Pain Points**:
- New concept for Git users
- Conflict UI needs refinement

### Darcs: Conflict Marking

Darcs (Haskell-based patch VCS) has **conflict marking**:

- Conflicts marked with special syntax
- Can have "hunks" that are conflicting
- Resolution is explicit

Similar to Pijul but older and less developed.

### Perforce: File Locking Prevents Conflicts

Perforce uses **exclusive file locking** to prevent conflicts:

```bash
p4 edit -l file.txt  # Lock file
```

While locked, others cannot edit. This **eliminates content conflicts** for locked files.

**Tradeoff**:
- No merge conflicts for locked files
- But: serialization bottleneck
- But: requires coordination

**For unlocked files**:
- Standard merge with conflict resolution required
- Must resolve before submit

### Plastic SCM: Semantic Merge Reduces Conflicts

Plastic's **semantic merge** reduces false conflicts:

- Understands code structure
- "Method A changed, Method B changed" = no conflict
- Only reports true semantic conflicts

Doesn't preserve conflicts, but reduces them through better understanding.

## Pattern Analysis

### Conflict Handling Approaches

| Approach | Systems | Conflicts as Data? | Shareable? | Deferrable? |
|----------|---------|-------------------|------------|-------------|
| Working directory | Git, Mercurial, SVN | No | No | No |
| Recorded resolution | Git rerere | Partial (local) | No | No |
| Patch conflicts | Pijul, Darcs | Yes | Yes | Yes |
| Conflict commits | Jujutsu | Yes | Yes | Yes |
| Prevention (locking) | Perforce | N/A | N/A | N/A |
| Reduction (semantic) | Plastic | N/A | N/A | N/A |

### Conflict Resolution Patterns

1. **Immediate resolution** — Must resolve before proceeding (Git default)
2. **Recorded resolution** — Remember and reuse (Git rerere)
3. **Deferred resolution** — Commit conflict, resolve later (Jujutsu)
4. **Collaborative resolution** — Share conflict, resolve together (Jujutsu potential)
5. **Prevention** — Lock to prevent conflicts (Perforce)

### Repeat Failures

1. **Lost merge progress** — Abandoned merges lose resolution work
2. **Conflict rediscovery** — Same conflicts resolved multiple times
3. **No conflict provenance** — Don't know why conflict occurred
4. **Binary conflicts** — No tooling for unmergeable files
5. **Large file conflicts** — Conflicts in large files are painful

## Frontier Work

### Emerging Approaches

- **AI-assisted resolution** — GitHub Copilot-style conflict resolution
- **Three-way merge UIs** — Better visual tools (VS Code, IntelliJ)
- **Structural merging** — Merge at AST level, not text

### Unexplored Territory

- **Probabilistic conflicts** — "This might conflict" warnings
- **Temporal conflicts** — "This will conflict when branch B merges"
- **Partial resolution** — Resolve some conflicts, leave others

## Convergence Implications

### Core Insight

Convergence's `superposition` should learn from Jujutsu and Pijul:

- **Conflicts are data** — Stored, addressable, versioned
- **Multiple variants** — Represent conflicting states
- **Provenance** — Know where each variant came from
- **Resolution as data** — How resolved, by whom, when

### Superposition Definition

Based on prior art:

```
superposition: {
  id: "superposition-123",
  target_path: "/src/component.rs",
  variants: [
    { source: "bundle-abc", content_hash: "sha256:..." },
    { source: "bundle-def", content_hash: "sha256:..." }
  ],
  resolution: null | { 
    resolved_content_hash: "sha256:...",
    resolver: "user@example.com",
    resolved_at: "2026-03-07T20:00:00Z",
    method: "manual|automated|third_way"
  }
}
```

### Recommended Direction

1. **Superpositions are first-class objects** — Like Jujutsu conflict commits
2. **Bundles can contain unresolved superpositions** — No forced resolution
3. **Resolution is explicit and recorded** — Provenance tracked
4. **Multiple resolution strategies** — Take A, take B, merge, custom
5. **Superpositions can be reopened** — Resolution is not final

### Comparison to Existing Systems

| Feature | Git Conflicts | Jujutsu Conflicts | Convergence Superpositions |
|---------|---------------|-------------------|---------------------------|
| Persisted | No | Yes | Yes |
| Shareable | No | Yes | Yes |
| Provenance | No | Limited | Full |
| Reopenable | N/A | Limited | Yes |
| In bundles | N/A | Yes | Yes |

### Tradeoffs Accepted

- **Complexity** — Superpositions add conceptual overhead
  - Mitigation: Good UX, hide when not needed

- **Bundle ambiguity** — Unresolved bundles can't be "used"
  - Mitigation: Clear visual distinction, resolution prompts

- **Storage cost** — Multiple variants stored
  - Mitigation: Content-addressing, garbage collection

## Open Questions

1. How are superpositions serialized on disk?
2. Can superpositions nest (conflicts within conflicts)?
3. What's the UI for resolving a superposition?
4. Can superpositions be partially resolved?
5. How do we prevent superposition accumulation (tech debt)?

## Prototype Needs

Before finalizing superposition semantics:

1. **Conflict storage format** — Test representation options
2. **Resolution UX** — Design and test resolution flows
3. **Collaboration scenario** — Test sharing conflicts

## References

- Dossiers: Git (rerere), Jujutsu (conflict commits), Pijul (patch theory), Perforce (locking)
- Source map: 003-conflict-representation-patterns (pending)
- Related tracks: Track 4 (bundle semantics)
