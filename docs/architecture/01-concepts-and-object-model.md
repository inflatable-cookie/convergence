# Concepts and Object Model

This document defines the core nouns of Convergence and the invariants that make them composable across large orgs.

## Design goals

- Capture is cheap and non-blocking.
- "Ready for consumption" is phased; a release is a designated consumption endpoint, not the default.
- Conflicts are preserved as data (superpositions), surfaced and resolved at the right phase.
- The central authority provides identity, access control, auditability, and consistent gate semantics.

## Core object types

### Repository

A repo is the top-level namespace for:
- a gate graph
- scopes
- lanes (organizational ownership/visibility partitions)
- bundles/releases produced by gates
- stored blobs/manifests and provenance

### Workspace

A workspace is a client-side working directory plus local metadata that can:
- take snaps from the filesystem
- compute diffs between local state and known base bundles
- publish snaps to the server
- materialize views of a bundle (optionally with overlays/superpositions)

### Snap

A `snap` is a point-in-time capture of workspace filesystem state.

**Research basis**: [Translation Memo 001](~/Dev/projects/convergence/docs/research/translation-memos/001-snap-semantics.md)

Invariants:
- A snap is not assumed buildable.
- A snap is immutable once created.
- A snap can be created without network access.

Capture model (informed by research):
- **Automatic capture** — Snaps are captured continuously (time-based and/or change-based), not just on explicit command
- **Optional message** — Message can be added at capture time or later (before publish)
- **Build status tracking** — Build status is metadata, not a gate

Minimum metadata:
- `snap_id` (ULID, time-sortable)
- `workspace_id`
- `created_at`
- `root_manifest_id` (see storage model)
- `trigger` — Why captured (automatic vs. explicit)
- optional `message` (free-form)
- `build_status` — Unknown/Pending/Success/Failure

See also: [Prototype: Automatic Snap Capture](./prototype-snap-capture.md)

### Publication

A `publish` creates a publication that submits a chosen snap to a gate within a scope.

Intent:
- "This is complete for my responsibility at this phase." (Not necessarily ready for release.)

Minimum metadata:
- `publication_id`
- `snap_id`
- `repo_id`
- `scope_id`
- `target_gate_id`
- `lane_id` (or derived)
- `publisher_identity`
- optional structured notes (what/why, risk, review hints)

### Gate

A gate is a policy boundary that:
- defines what inputs it accepts
- defines how to coalesce inputs into a bundle
- defines checks/approvals required for a bundle to be promotable

**Research basis**: [Translation Memo 002](~/Dev/projects/convergence/docs/research/translation-memos/002-gate-policy-model.md)

Gates are connected as a DAG that typically converges to a terminal gate for the primary release flow.

Gate characteristics (informed by research):
- **Server-authoritative** — Policy lives on and is enforced by the server
- **Configurable policy** — Not hardcoded (unlike Perforce stream types)
- **Produces bundles** — Gates consume publications/bundles, output bundles
- **Explicit promotion** — User-initiated `promote` operation with policy checking

See also: [Prototype: Linear Gate Chain](./prototype-gate-chain.md)

### Bundle

A `bundle` is the output artifact of a gate.

Invariants:
- Bundles are immutable.
- A bundle may contain unresolved superpositions.
- A bundle has a promotability status per gate policy.

Minimum metadata:
- `bundle_id`
- `produced_by_gate_id`
- `scope_id`
- `inputs` (publications and/or upstream bundles)
- `root_manifest_id`
- `provenance` (who/when/how; policies executed)
- `status` (e.g. promotable true/false + reasons)

Phase 3 MVP note:
- The first implementation uses a simplified bundle record (root manifest + publication ids) and does not yet compute a coalesced manifest from inputs.

### Promotion

`promote` is an operation that advances a bundle to a downstream gate.

Promotion is always policy-checked. A bundle that fails policy is not promotable; it can still exist and be inspected.

### Release

A `release` is a bundle that has been designated for consumption via a named release channel.

Notes:
- A release is typically cut from the terminal gate of the primary gate graph, but it is not required.
- A repo may allow releases from earlier gates (e.g., compatibility releases for older versions, feature-flagged distributions, emergency patches) as long as gate policy permits.

Releases commonly attach:
- build artifacts
- SBOM/attestations
- signatures
- changelog/notes

## Additional concepts

### Superposition

A superposition is a first-class conflict object representing multiple competing versions of the same logical item.

**Research basis**: [Translation Memo 003](~/Dev/projects/convergence/docs/research/translation-memos/003-superposition-as-data.md)

Key properties:
- it is preserved, not rejected
- it carries provenance for each variant
- it can be resolved later into a single chosen/merged result
- **resolutions are recorded** — who, when, how, why
- **resolutions can be reopened** — not final until release

Superpositions can exist in:
- workspaces (private)
- bundles (phase outputs)

See [04-superpositions-and-resolution.md](./04-superpositions-and-resolution.md) for detailed structure.

### Lane (breadth partition)

A lane defines the default breadth/visibility partition:
- who sees whose publications/bundles by default
- who is responsible for convergence at which gates

Lanes support organizational scaling by preventing "subscribe to everyone" workflows.

### Scope (depth partition)

A scope is a branch-like track for a feature/milestone/release train.

Scopes:
- flow through the same gate graph
- contain publications/bundles specific to that track

## Identity and authorization (brief)

The server is the authority on:
- identity
- lane membership
- permissions (publish/converge/promote)

All published objects carry signed/verified provenance.

---

## Research Integration

This document incorporates findings from the Comparative Research Program (g01.043-g01.045):

### Systems Studied

- **Git** — Object store, explicit staging, distributed model
- **Mercurial** — Revlog, phases (draft/public/secret), extensions
- **Perforce Helix Core** — Centralized, streams (gate precedent), file locking
- **Plastic SCM** — Hybrid model, semantic merge, visual branching
- **Jujutsu** — Working copy as commit, conflicts-as-data, operation log

### Key Research Insights

1. **Continuous capture** (Jujutsu precedent) — Snap should be automatic, not explicit
2. **Gate workflows** (Perforce streams precedent) — Promotion paths should be structured but configurable
3. **Conflict preservation** (Jujutsu, Pijul precedent) — Superpositions as first-class data with provenance

### Research Documents

- [Research Program Overview](~/Dev/projects/convergence/docs/research/README.md)
- [Specimen Dossiers](~/Dev/projects/convergence/docs/research/specimen-dossiers/)
- [Value Tracks](~/Dev/projects/convergence/docs/research/value-tracks/)
- [Translation Memos](~/Dev/projects/convergence/docs/research/translation-memos/)

### Prototypes

- [Automatic Snap Capture](./prototype-snap-capture.md) — Validate continuous capture UX
- [Linear Gate Chain](./prototype-gate-chain.md) — Validate gate policy and promotion flow
