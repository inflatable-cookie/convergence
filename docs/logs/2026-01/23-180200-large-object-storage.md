# Decision: Chunked Large-Object Storage And Policy-Driven Retention

Date: 2026-01-23 18:02:00Z

## Context

Convergence targets workflows where multi-gigabyte assets change constantly (game pipelines, audio/video editing, etc.).

The current local store writes whole-file blobs (content-addressed) per distinct file version.

This is correct for integrity and determinism, but:
- repeated large file changes will grow storage quickly
- distribution becomes expensive (upload/download whole files)
- we need a clear policy boundary between "capture everything" and "retain forever"

Convergence also differs from Git in intent:
- snaps are local, iterative capture
- publishing/bundles/gates are the "consumable" convergence points
- retention should be driven by gate policy and pinned artifacts

## Decision

Large objects are stored and distributed using chunked, content-addressed storage.

1) File content representation
- Files may be represented as a Merkle/DAG of chunks.
- A file entry in a manifest can reference either:
  - a whole-file blob (legacy)
  - a chunk-tree root (new)

2) Distribution
- Missing-object negotiation and transfer operate at chunk granularity.
- Upload/download is resumable and incremental.

3) Retention
- Retention is policy-driven.
- Snaps are not guaranteed to be retained forever.
- Bundles/releases (and explicitly pinned artifacts) define retention roots.

4) Gate policy
- Gates can enforce stricter object-availability requirements as artifacts move toward release.
- Early gates may accept metadata-first publications.
- Later gates require that referenced objects are present and fetchable.

## Consequences

- A multi-GB file can be snapped locally without loading it fully into memory.
- Small edits to large files can avoid re-storing/re-uploading the entire file.
- Distribution cost is proportional to changed chunks, not file size.
- We need explicit object lifecycle tooling (pinning, GC, policy configuration).

## Open Questions

- Chunking strategy: fixed-size vs content-defined chunking (CDC), and default chunk size.
- Whether to add pack/delta compression for text blobs in addition to chunking.
- When/where to support lazy materialization (restore-on-demand) vs eager restore.
- Server-side accounting/quotas and how gate policies interact with billing/storage limits.
