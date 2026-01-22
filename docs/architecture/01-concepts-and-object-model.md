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

Invariants:
- A snap is not assumed buildable.
- A snap is immutable once created.
- A snap can be created without network access.

Minimum metadata:
- `snap_id`
- `workspace_id`
- `created_at`
- `root_manifest_id` (see storage model)
- optional `message` (free-form)

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

Gates are connected as a DAG that typically converges to a terminal gate for the primary release flow.

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

Key properties:
- it is preserved, not rejected
- it carries provenance for each variant
- it can be resolved later into a single chosen/merged result

Superpositions can exist in:
- workspaces (private)
- bundles (phase outputs)

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
