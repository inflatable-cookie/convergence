# Phase 018: Large Object Storage, Retention, And Gate Policies

Current status:
- Implemented (fixed-size chunking MVP): chunked file entry type, local store ingest/restore, publish/fetch over recipes+chunks.

## Goal

Make large, frequently-changing files (game assets, audio, video, etc.) feasible in Convergence.

This phase focuses on two problems:
- what gets snapped (locally) and for how long
- how publications/bundles/releases handle large objects without destroying performance or distribution

## Why This Phase Exists

The current local store writes whole-file blobs per distinct version.

This is correct but not sufficient for environments where multi-GB assets change constantly.

We need structural deduplication (chunking), resumable distribution, and explicit retention policies tied to gates.

## Scope

In scope:
- Chunked (Merkle/DAG) representation for files, with stable content addressing.
- Local store support for chunked ingest + restore.
- Server and protocol support for chunk object distribution (missing-object negotiation, resumable upload).
- Policy-driven retention/GC for local and server stores.
- Gate-level policies for object availability (what must be present to publish/promote/release).

Out of scope:
- Full Git-style packfiles/delta chains for text blobs (nice follow-on; chunking is the baseline).
- UI polish and throughput tuning beyond correctness + basic ergonomics.

## Architecture Notes

Design targets:
- A multi-GB file can be snapped locally without loading it fully into memory.
- Small edits to a large file should result in only new chunks being stored/uploaded.
- Fetch/restore can be lazy (only materialize bytes when needed), but should support eager prefetch.
- Bundles/releases define retention roots; non-reachable objects can be garbage collected.

## Tasks

### A) Data model

- [x] Define chunk object format and hashing (fixed-size chunking; defaults: 4MiB chunks, 8MiB threshold).
- [x] Define a file "recipe"/"chunk tree" object that references chunks and reconstructs the file.
- [x] Update manifest file entries to reference either:
  - a whole-file blob (legacy), or
  - a chunk tree root (new)
- [x] Version/compat strategy so older snaps remain readable.

### B) Local store: chunked ingest + restore

- [x] Stream-chunk file reads (bounded memory), write chunk objects.
- [x] Write file recipe objects; update snap manifests accordingly.
- [x] Restore file bytes from chunk tree deterministically.
- [ ] Add tests:
  - [x] multi-chunk file roundtrip
  - [x] small edit causes limited new chunks
  - restore determinism across platforms

Defaults and configuration:
- Defaults: `chunk_size=4MiB`, `threshold=8MiB`.
- Workspace config: `.converge/config.json` supports `chunking: { chunk_size, threshold }`.
- TUI: `chunking show|set|reset` (local root context).

### C) Distribution protocol

- [x] Extend missing-object discovery to include chunk objects + recipe objects.
- [x] Implement resumable, chunk-level upload/download.
- [x] Ensure server can validate object availability for a publication/bundle.

### D) Gate policies for large objects

- [ ] Define per-gate object availability requirements:
  - early gates may allow "metadata-only" publications (manifest references accepted, bytes may be pending)
  - later gates require full availability of referenced objects
- [ ] Define a "pinned" concept for bundles/releases that prevents GC of required objects.

### E) Retention + GC

- [ ] Local retention policy config (keep last N snaps, keep last X days, keep pinned).
- [ ] Local mark/sweep GC that deletes unreferenced objects.
- [ ] Server retention policy that is driven by published bundles/releases.

## Exit Criteria

- A large file is stored as chunked content, not whole-file copies.
- Editing a small region of a large file only stores/uploads new chunks.
- Publishing/fetching works with chunked objects via missing-object negotiation.
- GC can safely delete unpinned/unreachable large objects locally and on the server.
