# Superpositions and Resolution

This document defines conflict representation as data.

## What is a superposition?

A superposition exists when multiple versions compete for the same logical path in the same view.

Examples:
- two publications modify `src/lib.rs` in incompatible ways
- two bundles both claim different contents for `assets/logo.png`

## Representation

At the manifest level, a path can map to either:
- a single entry (normal)
- a superposition entry (conflict)

In the current implementation, a directory tree is represented by a manifest graph.

A superposition is represented as a manifest entry:
- `ManifestEntryKind::Superposition { variants: Vec<SuperpositionVariant> }`

Each `SuperpositionVariant` contains:
- `source`: the publication id that contributed this variant
- `kind`:
  - `File { blob, mode, size }`
  - `Dir { manifest }`
  - `Symlink { target }`
  - `Tombstone`

## Where superpositions can exist

- Workspace view:
  - user can choose a default variant without resolving globally
- Bundle output:
  - a gate can emit a bundle containing superpositions
  - promotability can require resolving before promotion

## Resolution

Resolution is the act of collapsing a superposition to a single result.

Resolution types:
- Choose: select one variant (or tombstone).
- Merge: (planned) produce a new blob or derived artifact.

Resolution outputs:
- a new manifest graph where the path maps to the resolved entry
- (planned) provenance linking back to all variants

### Resolution MVP (current)

For now, resolutions are workspace-local and path-based:
- Stored as `.converge/resolutions/<bundle_id>.json`
- Decision format is `path -> variant_index` (0-based index into the variant list)

Applying a resolution:
- rewrites the bundle's `root_manifest` by replacing each superposition with the chosen variant
- yields a new `root_manifest` id
- writes a new local snap; optional publish of that snap produces a new publication that can be bundled

CLI:
- `converge resolve init|pick|clear|show|apply`

## UX constraints (large-org safe)

- Superpositions must be discoverable and inspectable.
- Superpositions must not explode the filesystem into unbounded alternate files.
- Resolution must be attributable (who resolved, what inputs were considered).

Suggested UX strategy:
- keep alternates in the object model
- materialize alternates into the filesystem only on demand (planned)
