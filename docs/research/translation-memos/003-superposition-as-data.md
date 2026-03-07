# Translation Memo 003: Superposition as Data

## Problem Statement

Convergence introduces `superposition` as a first-class conflict object. How should conflicts be represented, stored, and resolved?

Research question: Can conflicts be data rather than blockers? What prior art exists?

## External Evidence

### Research Findings (Track 3)

Six approaches compared:

| Approach | Persisted? | Shareable? | Reversible? | Systems |
|----------|-----------|-----------|-------------|---------|
| Working directory | No | No | No | Git, Mercurial, SVN |
| Recorded resolution | Partial | No | No | Git rerere |
| Patch conflicts | Yes | Yes | Yes | Pijul, Darcs |
| Conflict commits | Yes | Yes | Limited | Jujutsu |
| Locking prevention | N/A | N/A | N/A | Perforce |
| Semantic reduction | N/A | N/A | N/A | Plastic SCM |

Key finding: **Jujutsu and Pijul prove conflict-as-data is viable**.

### Key Insights from Research

1. **Immediate resolution loses work** — Abandoned merges lose resolution effort
2. **Conflicts aren't shareable** — Can't collaborate on resolution
3. **No conflict provenance** — Don't know why conflict occurred
4. **Resolution is final** — Can't reopen or reconsider
5. **Prevention (locking) has costs** — Serialization, coordination overhead

## Cross-System Comparison

### Conflict Representation

| System | Representation | Metadata | Collaboration |
|--------|---------------|----------|---------------|
| Git | Working directory markers | None | No |
| Jujutsu | Commit with multiple parents | Parents = sources | Can share commit |
| Pijul | Algebraic patch objects | Patch dependencies | Can share patches |
| **Convergence** | **Structured superposition** | **Full provenance** | **Collaborative resolution** |

### Resolution Patterns

| Pattern | When Resolved | Reversible? | Provenance? |
|---------|---------------|-------------|-------------|
| Immediate | Before commit | N/A | No |
| Recorded | At resolve time | No | Limited |
| Collaborative | Any time | Yes | Yes |

## Convergence Implications

### Recommended Superposition Model

```
superposition: {
  # Identity
  id: ulid,
  bundle_id: ulid,        # Which bundle contains this
  path: string,           # File path
  
  # Variants (conflicting states)
  variants: [
    {
      source: "bundle-abc",
      contributor: "user@example.com",
      content_hash: "sha256:...",
      description: "Added validation"
    },
    {
      source: "bundle-def", 
      contributor: "other@example.com",
      content_hash: "sha256:...",
      description: "Refactored input"
    }
  ],
  
  # Resolution (null if unresolved)
  resolution: {
    resolved_content_hash: "sha256:...",
    resolver: "user@example.com",
    resolved_at: "2026-03-07T20:00:00Z",
    method: "take_a" | "take_b" | "merge_manual" | "third_way",
    resolution_content_hash: "sha256:..."  # If manual
  } | null,
  
  # Status
  status: "unresolved" | "resolved" | "reopened"
}
```

### Key Decisions

1. **Superpositions are first-class objects** — Stored, addressable, versioned
2. **Full provenance** — Know where each variant came from
3. **Resolutions are recorded** — Who, when, how
4. **Resolutions can be reopened** — Not final
5. **Multiple resolution strategies** — Automated and manual

### Superposition in Bundles

Bundles can contain **unresolved superpositions**:

```
bundle: {
  id: ulid,
  status: "complete" | "partial",  # partial = has unresolved superpositions
  superpositions: [superposition_id],
  ...
}
```

This means:
- Gates can produce bundles with conflicts
- Bundles aren't blocked by conflicts
- Promotion may require resolution (gate policy)
- Releases require all superpositions resolved

### Resolution Workflow

```
# Bundle has superposition
$ converge show bundle-abc
Bundle: bundle-abc
Status: partial (1 unresolved superposition)

# View superposition
$ converge superposition show super-xyz
Path: src/auth.rs
Variants:
  [A] From bundle-def (Alice): Added validation
  [B] From bundle-ghi (Bob): Refactored input

# Resolve
$ converge superposition resolve super-xyz --strategy merge_manual
Opening editor for manual resolution...
Resolved. Bundle bundle-abc status: complete
```

### Collaborative Resolution

Superpositions can be **collaboratively resolved**:

1. Bundle with superposition is visible to team
2. Multiple users can view variants
3. Anyone can propose resolution
4. Gate policy may require specific resolver (e.g., "owner must approve")

## Tradeoffs Accepted

### Complexity

**Concern**: Superpositions add conceptual overhead

**Mitigation**:
- Hide superpositions when resolved
- Clear visual distinction in UI
- Simple cases should feel simple

### Bundle Ambiguity

**Concern**: Unresolved bundles can't be "used"

**Acceptance**: Correct. This is the point — bundles with conflicts are valid but limited.

**Mitigation**:
- Clear status indicators
- Resolution prompts at appropriate times
- Don't block workflow, just clarify state

### Storage Cost

**Concern**: Multiple variants stored

**Mitigation**:
- Content-addressing (shared storage)
- Garbage collection (resolved superpositions archived)
- Compression

## Open Questions

1. **Binary superpositions** — How to represent unmergeable file conflicts?
2. **Nested superpositions** — Can a resolution create a new superposition?
3. **Auto-resolution** — How much can be automated? (rerere-style, semantic)
4. **Superposition expiration** — Should old unresolved superpositions age out?

## Comparison to Existing Architecture

Current architecture describes superpositions but lacks detail:

| Aspect | Current Doc | This Memo Recommendation |
|--------|-------------|-------------------------|
| Storage | Mentioned | Structured object |
| Provenance | Mentioned | Full tracking |
| Resolution | Mentioned | Recorded, reversible |
| Collaboration | Not mentioned | Supported |

## Prototype Validation Needed

Before final adoption:

1. **Resolution UX** — Design and test resolution flows
2. **Collaborative scenario** — Test team conflict resolution
3. **Storage benchmark** — Measure cost of variant storage

## Recommended Next Step

**Outcome**: `promote to concept work`

Superposition-as-data is well-supported by research (Jujutsu precedent). Proceed to concept work:

1. Update `docs/architecture/04-superpositions-and-resolution.md` with detailed superposition structure
2. Define superposition storage format
3. Design resolution UX
4. Plan collaborative resolution features

## Architecture Updates Required

Update `docs/architecture/04-superpositions-and-resolution.md`:
- Add superposition data structure
- Document resolution mechanics
- Define superposition lifecycle

Update `docs/architecture/01-concepts-and-object-model.md`:
- Reference superposition definition
- Clarify bundle can contain unresolved superpositions

## References

- Value Track: [Track 3: Conflict Preservation](../value-tracks/conflict-preservation.md)
- Dossiers: Jujutsu (conflict commits), Pijul (patch theory), Perforce (locking)
- Architecture: `docs/architecture/04-superpositions-and-resolution.md`
