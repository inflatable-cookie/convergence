# 16 Sync Protocol and Chunking

Status: active
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.4

The wire contract between client and server, and the content-chunking scheme
both share. The protocol *shape* is carried from g01 (salvage inventory);
both endpoints are reimplemented.

## 1. Sync protocol

HTTP + JSON control messages, binary object bodies. DTOs live in
`converge-model` — one definition, no client/server drift (g01 duplicated
them by hand).

Publish/sync sequence (carried shape):

1. **Negotiate** — client sends the snap's object-ID set (root manifest walk);
   server (or edge) answers with the missing subset. Merkle short-circuit:
   a known manifest ID prunes its whole subtree from the set.
2. **Upload** — client streams missing blobs/recipes/manifests, then the snap
   record. All writes idempotent (`write_if_absent`); interrupted uploads
   resume by re-negotiating.
3. **Commit intent** — client creates the publication (or lane-head update)
   naming `(repo, scope, gate)` and carrying `base_bundle_id`, the last
   bundle it saw for that target (doc 17 §2; the client records it from
   publish responses and fetches). Server authz-checks, validates the base
   against partition history, and enqueues bundle coalescing (doc 14 §4-5).

Wire deltas from doc 17 §5: `SnapRecord` v2 (`parents`,
`derived_from_bundle`, lineage-derived identity), `base_bundle_id` on
publish/publication, `window` + `strategy` + `base_bundle_id` on
`BundleRecord`, `strategy` on `GateNode`.

Fetch is the mirror: resolve a bundle/release/lane ref → walk manifest →
request missing objects → materialize. Edges serve steps 1-2 from cache.

Versioning: protocol carries an explicit version; servers refuse unknown
majors. No silent compatibility shims pre-1.0.

## 1b. Canonical object encoding (g02.010)

Hashed stored objects (manifests, recipes) use a canonical binary
encoding; JSON remains the HTTP/API representation.

- encoding: CBOR (`ciborium`) of the model structs — serde field order is
  declaration order, collections are ordered types (Vec/BTreeMap), so the
  byte form is deterministic for identical values
- each object is prefixed with a 4-byte magic + version:
  `CVM1` (manifest), `CVR1` (recipe); decoders refuse unknown magics.
  Snap records keep JSON (their ids derive from structured fields via
  `compute_snap_id`, not from stored bytes)
- **hashing operates on the canonical bytes** (magic included), so object
  ids change from the JSON era — pre-1.0, stores re-init, no migration
- blobs remain raw bytes

Manifest paging for very large directories (>4096 entries) is **deferred
to backlog**: it touches every manifest walker for a case the beachhead
rarely hits; revisit against real trees.

## 1c. Batched transport (g02.010)

Object transfer moves in batches; the per-object routes remain valid
within the same wire version (additive change).

- `POST /api/repos/{repo}/objects/batch` — body: canonical CBOR sequence
  of `ObjectFrame { kind, id, bytes }`; server verifies each frame's hash
  on write (unchanged discipline); response reports the stored count
- `POST /api/repos/{repo}/objects/batch-get` — body: JSON `ObjectSet`;
  response: CBOR `ObjectFrame` sequence
- batches are size-capped (default 8 MiB); clients split
- server-side caps are part of the wire contract (g02.011): at most
  4096 frames per batch upload, at most 4096 requested ids per
  batch-get, 64 MiB request body limit — over-cap requests get a clear
  400 naming the cap, and clients split both uploads and fetches
- resumability is unchanged: batches are idempotent puts, and a failed
  batch is simply renegotiated

## 1d. Repo-scoped object access (g02.011)

Objects are content-addressed and deduped across repos, so possession of
a hash must not grant read access. All object and negotiate routes are
repo-scoped (`/api/repos/{repo}/…`) and authorized against that repo;
every server-side object write also records an object→repo association
in metadata.

- reads require the association: an object another repo uploaded is 404,
  indistinguishable from absent
- `negotiate` reports present-but-unassociated objects as **missing**;
  the client's idempotent re-put is cheap and repairs the association —
  cross-repo dedup still avoids re-storing bytes, never re-transfer
- bundle-id-keyed reads (`/api/bundles/{id}`, provenance, verify)
  resolve the bundle's repo and require `read` there; unauthorized and
  absent are both 404 so bundle ids are not an existence oracle
- GC sweep removes an object's associations with the object

## 2. Content-defined chunking

g01 used fixed 4 MB blocks (8 MB threshold) — weak dedup on inserts/edits.
Rebuild replaces it with **FastCDC** content-defined chunking:

- target chunk size ~1 MB (min ¼×, max 4× target) for large files;
  small files stay whole blobs
- chunk boundaries derive from content, so an insert shifts one chunk, not
  every subsequent block — dedup survives edits, which is the point for the
  large-binary-churn workloads Convergence targets
- recipe format: ordered chunk list `(chunk_id, length)` + total hash;
  recipes are objects like any other. The g01 recipe concept carries; the
  boundary algorithm and parameters are new, so g01 recipes are not migrated
  (archive is read-only history, not a data migration source)
- parameters (target size, normalization level) are recorded in the recipe
  header — future retuning cannot corrupt old recipes

## Next Task

Implement `converge-model` DTOs + FastCDC chunker early in the first rebuild
implementation roadmap; property-test chunking stability against
insert/delete edits.
