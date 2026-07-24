# 2026-07-24 Batch 12.2 Complete — GC Reachability Guarantee

Audit C2 (GC collects uploaded-but-unpublished objects once the 300s
mtime grace passes) and M4 (`put_snap` accepts a snap whose root
manifest was never uploaded) are closed; card 043, roadmap `g02.012`.

## What landed

- `object_pins(repo_id, kind, object_id)` in both metadata backends;
  `pin_object` / `unpin_object` / `is_object_pinned` (the last checks
  across repos, since the object store is shared)
- `AssociatingObjects` pins every object it writes — a fresh upload is
  protected before any bundle/publication/snap references it,
  independent of clock time
- pins released once the tree is durably referenced: `publish` unpins
  the publication's tree; `set_lane_head` unpins the head lineage's
  trees. Both are GC-reachable at that point, so unpinning never drops
  protection
- GC sweep skips pinned objects; the mtime grace is demoted to a
  micro-window guard for the sub-millisecond store-write→pin-write gap,
  no longer the load-bearing protection the audit flagged as a guess
- `upload_snap_record` verifies its root manifest object is present, so
  a snap (and a lane head fast-forwarded to it) can always materialize

## Validation

`effigy validate` green (110 tests, incl. `gc_upload_pins` at engine
level with zero grace: an unpinned orphan is swept while the pinned
upload survives, then publish succeeds and the tree stays reachable);
feature clippy clean.

## Next

Batch card 12.3: `pull_lane` distinguishes 404 from transient errors;
`upload_tree` child-first ordering + leaf re-verification.
