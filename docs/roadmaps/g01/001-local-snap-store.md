# Phase 001: Local Snap Store (MVP Foundation)

Current status:
- Implemented: local `.converge/` store, `converge init|snap|snaps|show|restore`
- Determinism: sorted manifests; restore recreates byte-identical trees
- Tests: roundtrip restore, blob corruption detection, manifest determinism

## Goal

Ship a working local-first Convergence CLI that can:
- initialize a workspace
- create immutable `snap`s from the filesystem
- list and inspect snaps
- restore a snap back into the working directory

This phase intentionally does not require a server or background file watcher; users explicitly run `converge snap`.

## Why This Phase Exists

- Establish the core data model (blobs/manifests/snaps) with strong determinism.
- Provide an end-to-end vertical slice that future phases (publish/gates/server/TUI) can build on.
- Validate the ergonomics of "snap without needing a working build".

## Scope

In scope:
- Rust project skeleton with a `converge` binary.
- Local content-addressed blob store.
- Tree manifests representing directory state.
- Snap creation from a working directory.
- Snap listing and inspection.
- Snap restore (materialize to working directory).
- Ignore rules (initially minimal; can mirror `.gitignore` semantics later).
- `--json` output for machine readability (at least for `status`/`list`/`show`).

Explicitly out of scope:
- Central authority server.
- `publish`, gates, bundles, promotion, release channels.
- Background file watching / IDE integration.
- TUI.
- Rich merge/superposition UX (conflict objects may be represented later).

## Architecture Notes

- The store should be content-addressed for blobs.
- Snaps should reference a root manifest.
- All IDs should be stable and deterministic.
- Restoring the same snap into an empty directory should produce byte-identical results.

Current on-disk layout (v1):
- `.converge/config.json`
- `.converge/objects/blobs/<blake3>`
- `.converge/objects/manifests/<blake3>.json`
- `.converge/snaps/<snap_id>.json`

## Tasks

### A) Repository + CLI skeleton

- [x] Create a Rust workspace (Cargo) and a `converge` binary.
- [x] Implement top-level command parsing and help output.
- [x] Implement `--json` output plumbing (even if only a subset of commands supports it initially).

### B) Local workspace metadata

- [x] Define the workspace config format (repo-independent for this phase).
- [x] Decide and document on-disk layout (e.g. `.converge/`).
- [x] Implement `converge init` to set up metadata and directories.

### C) Content-addressed blob store

- [x] Define blob hashing algorithm (e.g. BLAKE3, SHA-256) and ID representation.
- [x] Store blobs by hash and prevent duplication.
- [x] Implement integrity checks when reading blobs.

### D) Manifests (directory trees)

- [x] Define manifest encoding (e.g. CBOR/JSON) and hashing.
- [x] Support entry types:
  - [x] file (blob + metadata)
  - [x] dir (child manifest)
  - [x] symlink (optional; can be deferred)
- [x] Implement deterministic ordering and hashing.

### E) Snap creation

- [x] Walk the filesystem and build a manifest tree.
- [x] Store newly discovered blobs/manifests.
- [x] Create a snap record that points to the root manifest.
- [x] Implement `converge snap`.

### F) Listing / inspection

- [x] Implement `converge snaps` (list snaps, newest first).
- [x] Implement `converge show <snap-id>` (metadata + summary).

### G) Restore

- [x] Implement `converge restore <snap-id>`.
- [x] Define behavior for existing files (default: refuse unless `--force`, or restore into empty dir).

### H) Tests

- [x] Unit tests for hashing and manifest determinism.
- [x] Golden tests for restore determinism.

## Exit Criteria

- `converge init` creates `.converge/` and a workspace config.
- `converge snap` creates a new snap that can be listed.
- `converge restore <snap-id>` recreates the snap’s tree deterministically.
- At least minimal `--json` support exists for listing/inspection.

## Follow-on Phases

- Phase 002: Central Authority MVP (auth, publish intake, fetch) + object distribution.
- Phase 003: Gates + Bundles + Promotion semantics wired to the server.
- Phase 004: TUI for inbox/superpositions/bundle promotion.
- Phase 005: Background capture (daemon/IDE).
