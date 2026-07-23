# 003 Salvage Inventory

Status: capture artifact
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.1

Concrete carry/discard verdicts for the archive cut (Batch 2.2). Anything not
listed as carry is archived on `archive/g01` and removed from `main`.

## Carry: code

### `src/model/` → shared `model` crate (crown jewel)

- `ids.rs`, `manifest.rs`, `snap.rs`, `resolution.rs`, `config.rs`, `mod.rs`
- blake3 content-addressed IDs; `Manifest` with `ManifestEntryKind` =
  File / FileChunks / Dir / Symlink / `Superposition{variants}`;
  `compute_snap_id`
- Defines the on-disk and on-wire contract. Promote to standalone workspace
  crate; both client and server depend on it.

### `src/store/` → client crate (local object store)

- `core_setup.rs`, `object_crud/`, `snap_resolution/`, `state_meta/`,
  `traversal.rs`
- Verify-on-read hashing, atomic temp+rename writes, `write_if_absent` dedup,
  `.converge/` layout
- Caveat: flat object dirs — add sharded fanout (e.g. `objects/ab/cdef...`)
  during rebuild.

### `src/diff/` → client crate

- `diff_ops.rs`, `signatures.rs`, `tree_build.rs`, `walk.rs`
- Model-driven tree diff; portable as-is.

### `src/resolve/` → client crate

- `types.rs`, `validate.rs`, `variants.rs`, `apply/`
- Superposition resolution + validation; portable as-is.

### `src/workspace/` → client crate, partial

- Carry: `manifest_scan/`, `manifest_query.rs`, `snap_ops.rs`,
  `materialize_fs/`, `restore_materialize.rs`, `path_ops.rs`,
  `root_lifecycle.rs`, `gc/`
- Discard: `chunking.rs`, `chunk_io.rs` fixed-block scheme — replace with
  content-defined chunking (recipe format redesign expected).

## Carry: contracts (shape, not code)

- Sync protocol: client computes missing objects → uploads
  blobs/manifests/recipes/snaps → creates publications/lane-heads. Preserve as
  a documented wire contract; both endpoints get reimplemented.
- Wire DTO set (`src/remote/types/` vs server `types/repo/` duplication)
  collapses into the shared model crate in the rebuild.

## Discard: code

- `src/bin/converge_server/` entirely — esp. `persistence/` (whole-repo
  `repo.json` rewrite), `types/app_state.rs` (in-memory RwLock maps). Server
  rebuilds on real object store + metadata DB per Batch 2.4 architecture.
- `src/tui_shell/`, `src/tui.rs` — 16.5k LOC. UX captured in
  [`002-tui-ux-spec.md`](./002-tui-ux-spec.md); implementation archived.
- `src/remote/` client — blocking reqwest impl tied to dev-server API;
  protocol shape carries (above), code does not.
- `src/cli_commands/`, `src/cli_exec/`, `src/cli_subcommands/`,
  `src/cli_runtime.rs` — rebuilt against new client architecture; verb surface
  carries via the six-verb contract, dispatch code does not.
- `scripts/` node wrappers, `dev/` — tied to archived binaries.

## Carry: docs (keeper set for Batch 2.3)

- `docs/vision/001-convergence-platform-vision.md`
- `docs/architecture/01-concepts-and-object-model.md`
- `docs/architecture/04-superpositions-and-resolution.md`
- `docs/architecture/product-guardrails.md` (condensed)
- `docs/research/specimen-dossiers/` (5 dossiers)
- `docs/research/translation-memos/` (3 memos)
- `docs/research/value-tracks/` (3 tracks)
- `docs/git-podcast/summary.md` (origin rationale; raw transcript archived)
- `docs/rebuild/` (these capture artifacts)
- `docs/contracts/`, `docs/specs/`, `docs/roadmaps/g02/`, `docs/logs/` live
  planning surfaces continue per working rules; g01 roadmap files archive.

## Next Task

Open the Batch 2.2 archive-cut card once all three capture artifacts are
linked from the governing spec.
