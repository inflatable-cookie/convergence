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
   naming `(repo, scope, gate)`; server authz-checks and enqueues bundle
   coalescing (doc 14 §4-5).

Fetch is the mirror: resolve a bundle/release/lane ref → walk manifest →
request missing objects → materialize. Edges serve steps 1-2 from cache.

Versioning: protocol carries an explicit version; servers refuse unknown
majors. No silent compatibility shims pre-1.0.

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
