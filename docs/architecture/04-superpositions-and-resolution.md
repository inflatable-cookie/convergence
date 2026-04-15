# Superpositions and Resolution

This document defines conflict representation as data.

**Research basis**: [Track 3: Conflict Preservation](~/Dev/projects/convergence/docs/research/value-tracks/conflict-preservation.md), [Translation Memo 003](~/Dev/projects/convergence/docs/research/translation-memos/003-superposition-as-data.md)

## What is a superposition?

A superposition exists when multiple versions compete for the same logical path in the same view.

Examples:
- two publications modify `src/lib.rs` in incompatible ways
- two bundles both claim different contents for `assets/logo.png`

Unlike Git (where conflicts block commits), Convergence treats superpositions as **first-class data** that can be:
- Stored and versioned
- Shared between users
- Resolved later (not immediately)
- Reopened after resolution

## Representation

### Superposition Structure

```rust
struct Superposition {
    // Identity
    id: Ulid,                    // Time-sortable unique ID
    bundle_id: Ulid,             // Which bundle contains this
    path: String,                // File path within bundle
    
    // Variants (conflicting states)
    variants: Vec<SuperpositionVariant>,
    
    // Resolution (null if unresolved)
    resolution: Option<Resolution>,
    
    // Status
    status: SuperpositionStatus,
}

struct SuperpositionVariant {
    source: Source,              // Where this variant came from
    contributor: Identity,       // Who contributed this
    content_hash: ContentHash,   // Content-addressed
    description: Option<String>, // Intent/context (e.g., commit message)
    timestamp: DateTime,         // When contributed
}

struct Source {
    bundle_id: Ulid,
    publication_id: Option<Ulid>,
}

struct Resolution {
    resolved_content_hash: ContentHash,
    resolver: Identity,          // Who resolved
    resolved_at: DateTime,       // When resolved
    method: ResolutionMethod,    // How resolved
    rationale: Option<String>,   // Why this resolution
}

enum ResolutionMethod {
    TakeA,           // Choose variant A
    TakeB,           // Choose variant B
    TakeN(usize),    // Choose variant N
    MergeManual,     // Manual merge produced new content
    Automated,       // Automated merge succeeded
    ThirdWay,        // Neither variant (custom content)
}

enum SuperpositionStatus {
    Unresolved,
    Resolved,
    Reopened,        // Was resolved, now reopened for reconsideration
}
```

### Manifest Level

At the manifest level, a path can map to either:
- a single entry (normal)
- a superposition entry (conflict)

```rust
enum ManifestEntryKind {
    File { blob, mode, size },
    Dir { manifest },
    Symlink { target },
    Tombstone,
    Superposition { 
        superposition_id: Ulid,  // Reference to full superposition record
    },
}
```

## Where superpositions can exist

### Workspace View

- User can view all variants of a superposition
- User can choose a default variant for their workspace without resolving globally
- Resolution in workspace is local-only (doesn't affect bundle)

### Bundle Output

- A gate can emit a bundle containing superpositions (status: `partial`)
- Bundles with superpositions are valid but limited:
  - Cannot be promoted to certain gates (policy-dependent)
  - Cannot be released (releases require all superpositions resolved)
- Promotion can require superpositions be resolved

### Release Channel

- Releases MUST have all superpositions resolved
- Release bundles have status: `complete`

## Resolution

### Resolution Types

1. **Choose** — Select one variant
   - `TakeA`, `TakeB`, `TakeN(n)`
   - Simple but loses other variant content

2. **Merge** — Produce new content combining variants
   - `MergeManual` — User manually merged
   - `Automated` — Automated merge succeeded
   - Preserves intent from both variants

3. **Third Way** — Neither variant
   - `ThirdWay` — Custom content that differs from all variants
   - May indicate different approach discovered

### Resolution Provenance

Every resolution records:
- **Who** — Identity of resolver
- **When** — Timestamp
- **How** — Resolution method
- **Why** — Optional rationale

This enables:
- Attribution and accountability
- Audit trails
- Learning from resolution patterns

### Reopening

Resolutions can be **reopened**:

```rust
// Resolution was: TakeA
// Reopen for reconsideration
superposition.status = SuperpositionStatus::Reopened;
superposition.resolution = None;  // Or keep as "previous_resolution"

// Can resolve differently
```

Use cases:
- Discovered resolution was incorrect
- New information changes best choice
- Policy changes require re-evaluation

## Storage

### Superposition Records

Superpositions are stored as first-class objects:

```
.converge/objects/superpositions/<ulid>.json
```

Format:
- JSON for readability/debuggability
- Content-addressed by superposition ID
- Immutable once created

### Resolution Storage

Resolutions are stored as part of the superposition record.

When a superposition is resolved, the bundle containing it is updated:
- New root manifest with resolved content
- Previous bundle retained (immutable history)
- Resolution provenance preserved

## UX Constraints (Large-Org Safe)

- **Superpositions must be discoverable and inspectable** — Clear UI indicators
- **Superpositions must not explode the filesystem** — Don't materialize all variants by default
- **Resolution must be attributable** — Who, when, how recorded
- **Resolutions can be reconsidered** — Not final until release

### Suggested UX Strategy

- Keep alternates in the object model
- Materialize alternates into filesystem only on demand
- Show superposition status in TUI with clear indicators
- Provide resolution workflow with variant comparison

## Research-Informed Design Decisions

### Why First-Class Superpositions?

Based on research (Jujutsu, Pijul):

1. **Deferred resolution** — Users can think before resolving
2. **Collaborative resolution** — Teams can resolve together
3. **Test before resolve** — Can run tests on conflicting state
4. **No lost work** — Abandoned merges don't lose resolution effort

### Why Full Provenance?

Based on large-org requirements:

1. **Accountability** — Know who made decisions
2. **Audit** — Regulatory/compliance needs
3. **Learning** — Understand resolution patterns

### Why Reopenable?

Based on real-world complexity:

1. **Mistakes happen** — Can fix incorrect resolutions
2. **New information** — Requirements change
3. **Policy evolution** — Gates may change resolution requirements

## Implementation Phases

### Phase 1 (Current)

- Basic superposition structure
- Workspace-local resolutions
- Simple pick-one resolution

### Phase 2 (Planned)

- Full provenance tracking
- Resolution recording
- Collaborative resolution (server-side)

### Phase 3 (Future)

- Reopenable resolutions
- Automated resolution suggestions
- Resolution pattern analytics

## CLI

Current:
- `converge resolve init|pick|clear|show|apply`
- `converge resolve pick --variant <n>` or `--key <json>`
- `converge resolve validate --bundle-id <id>`

Planned:
- `converge superposition list --bundle-id <id>`
- `converge superposition show <superposition-id>`
- `converge superposition resolve <superposition-id> --method <method>`
- `converge superposition reopen <superposition-id> --rationale <text>`

## Related Documents

- [Track 3: Conflict Preservation](~/Dev/projects/convergence/docs/research/value-tracks/conflict-preservation.md) — Research synthesis
- [Translation Memo 003](~/Dev/projects/convergence/docs/research/translation-memos/003-superposition-as-data.md) — Design rationale
- [01-concepts-and-object-model.md](./01-concepts-and-object-model.md) — Core object definitions
