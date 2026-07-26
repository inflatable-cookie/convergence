# 16 Sync Protocol and Chunking

Status: active
Updated: 2026-07-25
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
   the server answers with the missing subset. Merkle short-circuit:
   a known manifest ID prunes its whole subtree from the set.
2. **Upload** — client streams missing blobs/recipes/manifests, then the snap
   record. All writes idempotent (`write_if_absent`); interrupted uploads
   resume by re-negotiating.
3. **Commit intent** — client creates the publication (or lane-head update)
   naming `(repo, scope, gate)` and carrying `base_bundle_id`, the last
   bundle it saw for that target (doc 17 §2; the client records it from
   publish responses and fetches). Server authz-checks, validates the base
   against partition history, and builds the bundle in-request — the
   response carries the finished bundle (doc 14 §4-5; async coalescing is
   deferred, see doc 14 §7).

Wire deltas from doc 17 §5: `SnapRecord` v2 (`parents`,
`derived_from_bundle`, lineage-derived identity), `base_bundle_id` on
publish/publication, `window` + `strategy` + `base_bundle_id` on
`BundleRecord`, `strategy` on `GateNode`.

Fetch is the mirror: resolve a bundle/release/lane ref → walk manifest →
request missing objects → materialize. When edge nodes exist they will
serve steps 1-2 from cache; none are built (doc 14 §7).

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

## 1e. Paged listings (g02.015)

Every listing endpoint is a cursor page, so no response is unbounded:

- request: `?after=<cursor>&limit=<n>`; response:
  `{ items: [...], next_cursor: "<cursor>" | absent }`
- `limit` is clamped server-side to 1000 whether or not the client sends
  it — an old client cannot pull an unbounded set
- ordering is by a stable key (lane id, scope id, release seq), so a
  cursor never skips or repeats an item when rows are inserted
  concurrently
- a page that fills exactly still carries a cursor: the server does not
  spend a second query proving the listing ended, so a follower learns
  it from the next short or empty page
- the event feed predates this shape and keeps its own
  (`{events, floor, gap}`, doc 14 §5b) because it carries pruning
  information a plain cursor page has no place for

The inbox is a composite report, not a listing: each section is capped
and the report sets `truncated` when a cap cut it. It reads at most one
bundle per gate rather than scanning the scope.

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

## 3. On-disk format versioning (g02.022 batch 22.2)

`WIRE_VERSION` (§1) covers what two processes say to each other. This
covers what a process says to a directory it will read again later.

Both stores carry a version stamp: `.converge/format` in a workspace and
`format` in a server's data directory, each holding one line —
`converge-workspace-1`, `converge-server-1`.

### Why its own file

`WorkspaceConfig` has carried a `version` field since the rebuild and
nothing ever read it. Worse, it could not have worked: `config.json` is
parsed by serde, so a format change that alters its *shape* fails to
parse before anything looks at the version. The error would be "missing
field", not "wrong version".

A version stamp has to be readable by every binary that will ever meet
it, including ones written after the format it stamps. So it is a
standalone file holding text, and it will stay that way.

### Absent means 1

A store written before the stamp existed has none. Absent is defined as
version 1, permanently, and nothing rewrites it — so opening a store
stays a pure read. That property is load-bearing: `converge doctor`
opens a workspace and is tested to change nothing, and a
migrate-on-open would have made the diagnostic a mutation.

Version 2 onwards must write the file.

### What requires a bump

The test is **would a binary at the other version misread this**, not
"did the bytes change".

Not a bump:

- adding a file or directory that older readers do not look for
- adding an optional field older readers skip with `#[serde(default)]`
- anything under a path that is already ignored

A bump:

- changing what an existing field or file *means*
- changing an id's domain tag, which changes identity — batch 18.3 moved
  `converge-snap-v3` to `v4` and would have needed one
- removing something an older reader requires
- changing the layout an older writer would write into

### Both directions are refused

An older binary opening a newer store is the more dangerous case: it is
the one that reads fields whose meaning changed underneath it. It is
also the one people hit, because downgrading is what you do when a new
version misbehaves.

The refusal names the version found, the version supported, what to do,
and — because it happens on *open*, before anything is read — that
nothing has been touched.

### `--force` is not a licence to destroy

`converge init --force` refuses a store this build cannot read. Driving
this found the hole: every verb refused a format-99 workspace, and then
`init --force` reset it to format 1, destroying exactly the history the
refusal existed to protect. `--force` means "re-initialise over my own
store". Discarding one you cannot read means removing the directory
yourself, which is an unmistakable act rather than a flag people reach
for casually.

## Next Task

Implement `converge-model` DTOs + FastCDC chunker early in the first rebuild
implementation roadmap; property-test chunking stability against
insert/delete edits.
