# 043 GC Reachability Guarantee

Status: complete
Updated: 2026-07-24
Roadmap: `g02.012`
Spec: `docs/specs/012-data-safety.md`

## Objective

Audit C2 (GC collects uploaded-but-unpublished objects once the 300s
mtime grace passes) and M4 (`put_snap` accepts a snap whose root
manifest was never uploaded) closed.

## In Scope

- `object_pins(repo_id, kind, object_id)` table in both metadata
  backends; `pin_object` / `unpin_object` / `is_object_pinned`
  (`is_object_pinned` answers across repos — the store is shared)
- every server-side object write (via `AssociatingObjects`) pins the
  object: a freshly uploaded object is protected before any reference
  exists, independent of clock time
- pins released when the object becomes durably referenced: after a
  successful `publish`, unpin the objects reachable from the
  publication's tree; after `set_lane_head`, unpin the head lineage's
  trees — those are now GC-reachable, so unpinning never drops
  protection
- GC sweep skips any pinned object; the mtime grace stays only as a
  micro-window guard for the store-write→pin-write gap, no longer the
  load-bearing protection
- `upload_snap_record` verifies its `root_manifest` object is present
  (M4), so a snap — and therefore a lane head fast-forwarded to it —
  can always materialize
- tests: upload a tree without publishing, GC with zero grace, objects
  survive; then publish succeeds; after publish + GC the same objects
  become reclaimable once unreferenced; `put_snap` with an absent root
  manifest refused

## Out Of Scope

- abandoned-pin reaper for junk uploads never published (backlog, a
  bounded leak, not a correctness bug); sync failure honesty (12.3)

## Acceptance Criteria

- upload → zero-grace GC → publish keeps every object under test;
  snap upload without its tree refused; all suites green

## Validation

- `effigy validate`

## Outcome

- `object_pins` table in both backends; `pin_object` / `unpin_object`
  / `is_object_pinned` (global check — shared store), conformance-
  covered including two-repo pin/unpin
- `AssociatingObjects` pins every object it writes, so a fresh upload
  is protected before any reference exists — independent of clock time
- pins released once the tree is durably referenced: `publish` unpins
  the publication's tree, `set_lane_head` unpins the head lineage's
  trees (both now GC-reachable, so unpinning drops no protection)
- GC sweep skips pinned objects; the mtime grace is demoted to a
  micro-window guard for the store-write→pin-write gap
- `upload_snap_record` verifies its root manifest object is present
  (audit M4) — no more dangling lane heads
- engine-level regression (`gc_upload_pins`): upload without publish →
  zero-grace GC sweeps an unpinned orphan but keeps the pinned tree →
  publish succeeds → tree still reachable after a second GC; 110
  workspace tests green

## Next Task

Batch card 12.3 (honest sync failure).
