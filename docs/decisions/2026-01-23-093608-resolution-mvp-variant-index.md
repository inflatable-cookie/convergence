# Decision: Resolution MVP Uses Local Files + Variant Index

Date: 2026-01-23 09:36:08Z

## Context

Convergence represents conflicts as `superpositions` inside manifest graphs.

To unblock promotion and to validate end-to-end semantics, we need a minimal resolution workflow that:
- allows explicit user choice
- is deterministic
- does not require server-side resolution objects yet

## Decision

For the Resolution MVP:

- Resolutions are stored locally in the workspace under `.converge/resolutions/<bundle_id>.json`.
- A resolution records decisions as `path -> variant_index` (0-based index into the superposition variant list at that path).
- Applying a resolution rewrites the bundle root manifest deterministically and produces a new snap.
- The resolved snap can be published as a new publication.

CLI and TUI both operate on this same resolution file.

## Consequences

- Variant indices are not stable if the upstream coalescing algorithm changes variant ordering; this is acceptable for the MVP.
- Future work should define stable variant keys (likely derived from `{source_publication_id, kind, object ids}`) and a versioned resolution schema.
