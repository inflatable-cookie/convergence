# Translation Memo 001: Snap Semantics

## Problem Statement

Convergence needs a `snap` concept that captures workspace state. What exactly is a snap?

Research question: Should snaps be:
- Like Git commits (explicit, with message)?
- Like Jujutsu working copies (automatic, describe later)?
- Like editor auto-saves (continuous, no semantics)?
- Something new?

## External Evidence

### Research Findings (Track 1)

Five systems were compared:

| System | Capture Model | Intent Required | Build Guarantee |
|--------|---------------|-----------------|-----------------|
| Git | Two-stage explicit (stage + commit) | High | No |
| Mercurial | One-stage explicit | Medium | No |
| Jujutsu | Automatic (working copy = commit) | Low | No |
| Fossil | Explicit with auto-sync | Medium | No |
| Editors | Time-based auto-save | None | No |

Key finding: **No system guarantees buildable state at capture time**. Capture and buildability are separate concerns.

### Key Insights from Research

1. **Explicit capture loses work** — Users forget to commit, lose hours of work
2. **Automatic capture without context loses intent** — "What was I thinking?"
3. **Staging complexity confuses** — Git's index is powerful but hard to learn
4. **Describe-later works** — Jujutsu shows you can capture first, describe later

## Cross-System Comparison

### Capture Frequency

| Approach | Frequency | Risk of Loss | Intent Preservation |
|----------|-----------|--------------|---------------------|
| Explicit | Manual | High | High |
| Auto-save | Time-based | Low | Low |
| Working-copy-as-commit | Change-based | Very low | Medium |
| **Convergence snap** | **TBD** | **TBD** | **TBD** |

### Tradeoff Analysis

**Explicit capture (Git)**:
- ✅ Clear intent
- ✅ Meaningful history
- ❌ Lost work common
- ❌ Cognitive overhead

**Automatic capture (Jujutsu)**:
- ✅ No lost work
- ✅ Lower cognitive overhead
- ❌ History volume
- ❌ Intent may lag

## Convergence Implications

### Recommended Snap Semantics

```
snap: {
  # Identity
  snap_id: ulid,           # Time-sortable unique ID
  workspace_id: uuid,      # Where captured
  
  # Content
  root_manifest_id: hash,  # Content-addressed tree
  
  # Metadata (optional at capture)
  message: string | null,  # User intent (can be added later)
  
  # Automatic metadata
  created_at: timestamp,   # Capture time
  build_status: unknown | pending | success | failure,
  test_status: unknown | pending | success | failure,
  
  # Provenance
  parent_snap_id: ulid | null,  # Previous snap in workspace
}
```

### Key Decisions

1. **Capture is automatic** — Like Jujutsu, snaps occur without explicit user action
2. **Message is optional** — Can be added later (before publish)
3. **Build status is metadata** — Captured, but doesn't block snap
4. **Snaps are local** — Not shared until `publish`

### UX Implications

```
# User experience
$ converge status
Workspace: my-project
Snap: 01HQ... (2 minutes ago)
  Message: (none)
  Build: succeeded
  Changes: 3 files modified

$ converge snap --message "Add authentication"
# Adds message to most recent snap (no new snap)
```

### Relationship to Other Concepts

```
workspace state → [auto-capture] → snap → [explicit] → publish → gate → bundle
```

- **Workspace** has current state
- **Snap** captures state automatically
- **Publish** is explicit user intent to share
- **Gate** processes published snaps
- **Bundle** is gate output

## Tradeoffs Accepted

### Snap Volume

**Concern**: Automatic capture creates many snaps

**Mitigation**:
- Content-addressed storage (deduplication)
- Compression (like Git packfiles)
- Automatic pruning (keep last N per workspace)
- Snap "squash" before publish (optional)

### Intent Lag

**Concern**: Users may forget to add messages before publish

**Mitigation**:
- UI reminder for snaps without messages
- AI-assisted message suggestions
- Can add message to any snap before publish

### No Build Guarantee

**Concern**: Snaps may capture broken state

**Acceptance**: This is correct. Snaps capture reality. Build status is metadata that evolves.

## Open Questions

1. **Capture trigger** — Time-based (every N minutes)? Change-based (after idle)? Hybrid?
2. **Message editing** — Can messages be changed after added? (Probably yes until publish)
3. **Snap visibility** — Should snap history be navigable? (Yes, like local history)
4. **Cross-workspace snaps** — Related snaps across workspaces?

## Prototype Validation Needed

Before final adoption:

1. **Capture frequency study** — What frequency feels right?
2. **Storage benchmark** — How much storage at scale?
3. **UX testing** — Do users understand snap vs. publish?

## Recommended Next Step

**Outcome**: `prototype first`

Create a prototype implementing:
1. Automatic snap capture (configurable frequency)
2. Optional message attachment
3. Build status tracking
4. Snap history navigation

Test with real users before committing to architecture.

## References

- Value Track: [Track 1: Continuous Capture vs. Explicit Commit](../value-tracks/continuous-capture-vs-explicit-commit.md)
- Dossiers: Git, Jujutsu, Mercurial
- Architecture: `docs/architecture/01-concepts-and-object-model.md` (snap definition)
