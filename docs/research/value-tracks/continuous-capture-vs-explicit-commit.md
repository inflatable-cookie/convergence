# Track 1: Continuous Capture vs. Explicit Commit

## Problem Statement

Version control systems differ fundamentally in when and how they capture workspace state:

- **Explicit commit** (Git, Mercurial): User must consciously decide what to record
- **Continuous capture** (Fossil auto-sync, Jujutsu, editors): State is recorded automatically or implicitly

Convergence introduces `snap` as a point-in-time capture. Is it:
- Like Git's explicit commit with message and intent?
- Like Jujutsu's automatic working-copy-as-commit?
- Like editor auto-save without semantics?
- Something new?

## Cross-System Comparison

### Git: Explicit Staging and Commit

Git uses a **two-stage explicit model**:

1. **Staging** (`git add`) — Select changes for next commit
2. **Commit** (`git commit`) — Record with message

The **index** (staging area) allows precise control:
```
# Stage part of a file
git add -p file.txt

# Stage different hunks separately
git add -p  # interactive
```

**Strengths**:
- Precise control over what gets recorded
- Logical commits with clear intent
- Bisect and history are meaningful

**Pain Points** (from dossier):
- "WIP commits" are common but messy
- Lost work when users forget to commit
- Staging complexity confuses beginners
- No continuous capture — must remember to commit

### Mercurial: Simpler Explicit Model

Mercurial has **no staging area** by default:

```
# Modify files
# ...all modifications are implicitly "staged"
hg commit -m "message"
```

Extensions add sophistication:
- `crecord` — Interactive chunk selection
- `shelve` — Stash-like WIP preservation
- `absorb` — Automatically amend changes to prior commits

**Strengths**:
- Simpler mental model (no index)
- `absorb` enables automatic "fixup" behavior
- `shelve` for WIP without messy commits

**Pain Points**:
- Less control than Git staging
- Shelve is opt-in, not automatic

### Jujutsu: Working Copy as Commit

Jujutsu's radical model: **your working directory IS a commit**:

```bash
# No explicit "add" needed
$ echo "hello" > file.txt
$ jj st
Working copy : kmtnuoml 2f0d1b3e (no description set)
Parent commit: orrkiroy fd808 (main) Add test script

# The change is already "committed" to the working copy commit
```

The working copy commit:
- Is automatically created/amended
- Has no description initially
- Can be evolved (new commit, auto-rebase descendants)

**Strengths**:
- No lost work — everything is recorded
- No staging confusion
- Can describe changes when ready

**Pain Points**:
- New mental model requires unlearning
- "Commit" means something different

### Fossil: Auto-sync Option

Fossil offers **auto-sync** mode:

```bash
fossil settings autosync 1
```

When enabled:
- Commits automatically push to server
- Pulls automatically on update

But commits are still **explicit** — just sync is automatic.

### Editor Auto-save

Modern editors (VS Code, JetBrains) offer **auto-save**:

- Saves files automatically after delay
- No explicit save action needed
- File system state drifts from "intentional" state

**Characteristics**:
- Continuous, no user action
- No semantic meaning
- Often excluded from VCS (temp files, `.vscode/`)

## Pattern Analysis

### Common Patterns

| Pattern | Systems | Capture Trigger | User Intent |
|---------|---------|-----------------|-------------|
| Two-stage explicit | Git | `add` + `commit` | High precision |
| One-stage explicit | Mercurial | `commit` | Medium precision |
| Working copy as commit | Jujutsu | Auto on change | Automatic |
| Auto-sync | Fossil | Auto after commit | Network sync |
| Auto-save | Editors | Time-based | No semantics |

### Repeat Failures

1. **Lost work** — Users forget to commit before switching tasks
2. **WIP pollution** — Messy "checkpoint" commits clutter history
3. **Staging confusion** — Git's index is powerful but complex
4. **Intent loss** — Auto-capture without context loses meaning
5. **Binary bloat** — Capturing large files continuously is expensive

## Frontier Work

### Emerging Approaches

- **Jujutsu's model** — Working copy as commit, describe later
- **AI-assisted commits** — Auto-suggest commit messages from changes
- **Intent detection** — Classify changes as "WIP" vs. "ready"
- **Selective continuous** — Auto-capture small changes, explicit for large

### Unexplored Territory

- **Build-aware capture** — Only capture when build succeeds
- **Test-aware capture** — Only capture when tests pass
- **Semantic capture** — Capture at logical boundaries (function complete)

## Convergence Implications

### Key Questions for Convergence

1. **Is a snap buildable?**
   - Git commit: No guarantee
   - Jujutsu working copy: No guarantee
   - Convergence: ?

2. **Does a snap have a message?**
   - Git commit: Required
   - Jujutsu: Optional (can be empty)
   - Convergence: ?

3. **Who sees a snap?**
   - Git commit: Local until push
   - Jujutsu: Local until push
   - Convergence: ?

4. **Is capture explicit or automatic?**
   - Git: Explicit
   - Jujutsu: Automatic
   - Convergence: ?

### Recommended Direction

Based on cross-system analysis:

1. **Make snap automatic but lightweight** — Like Jujutsu's working copy, but even lighter
2. **Require message at publish time, not capture** — Separate capture from intent
3. **Build/test status is metadata, not gate** — Record status, don't block capture
4. **Local snaps are private, published snaps are shared** — Clear visibility boundary

### Tradeoffs Accepted

- **History volume** — More frequent capture = more history
  - Mitigation: Snap compression, automatic pruning
  
- **Noise vs. signal** — Automatic capture may record broken states
  - Mitigation: Metadata distinguishes "checkpoint" from "milestone"

- **Storage cost** — Continuous capture needs efficient storage
  - Mitigation: Content-addressing, delta compression like Git packfiles

## Open Questions

1. How often should automatic snap occur? (Time-based? Change-based?)
2. Should snaps be immutable once created?
3. How do we handle large files in continuous capture?
4. What's the UI for "these snaps are related" vs. "these are separate"?

## Prototype Needs

Before finalizing snap semantics:

1. **Frequency experiment** — Test time-based vs. change-based capture
2. **Storage benchmark** — Measure cost of continuous capture at scale
3. **UX study** — Do users prefer explicit or automatic capture?

## References

- Dossiers: Git (staging area), Mercurial (shelve), Jujutsu (working copy as commit)
- Source map: 001-git-internals-and-object-model (pending)
- Related tracks: Track 7 (workspace state management)
