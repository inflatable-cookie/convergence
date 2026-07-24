# 035 Batched Transport

Status: complete
Updated: 2026-07-24
Roadmap: `g02.010`
Spec: `docs/specs/010-scale-and-transport.md`

## Objective

Uploads and downloads move in batches instead of per-object round trips.

## In Scope

- wire (doc 16 amendment first): `POST /api/objects/batch` — CBOR body:
  sequence of (kind, id, bytes) frames; response per-frame ok/error;
  `POST /api/objects/batch-get` — request ids by kind, response frames;
  both size-capped (default 8 MiB per batch, client splits)
- client `upload_tree`/`fetch_manifest_tree`/`push_lineage`/`pull_lane`
  use batches (negotiate unchanged); per-object routes remain for
  compatibility within the version (same WIRE_VERSION — additive)
- resumability preserved: batch failure -> renegotiate picks up the
  delta (already idempotent)
- tests: e2e suites green over batched paths; a large-tree publish
  round-trips; batch cap splitting exercised (small cap in test)

## Out Of Scope

- events (10.3), external backends (10.4)

## Acceptance Criteria

- all sync e2e green via batches; splitting tested

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- wire ambiguity — doc 16 first

## Outcome

- doc 16 §1c amended first; `ObjectFrame` wire type (serde_bytes-backed)
- server: `POST /api/objects/batch` (CBOR frames, per-frame hash
  verification unchanged) and `/api/objects/batch-get` (JSON ObjectSet
  in, CBOR frames out); per-object routes retained (additive change,
  same WIRE_VERSION)
- client: cap-split `put_frames` (default 8 MiB, `with_batch_cap` test
  hook), upload_tree batches recipes/blobs then manifests-last;
  `fetch_manifest_tree` rewritten as a batched wave walk (manifests by
  level -> recipes -> all blobs); per-object client methods removed
- cap-splitting test: 64-byte cap forces per-frame batches over a
  22-object tree, publish + fetch + materialize round-trip green;
  all sync e2e suites now run over batches
- 94 workspace tests green

## Next Task

Execute the Batch 10.3 event-push card (`036-event-push.md`).
