# 2026-07-24 Batch 12.1 Complete — Safe Restore

Audit D1 (restore clears the workspace before knowing the target
materializes) and D2 (path traversal from untrusted manifests) are
closed; card 042, roadmap `g02.012`, spec 012.

## What landed

- `materialize_via_temp`: the tree is built into a temp dir inside the
  destination (same filesystem, so the final swap is renames); only on
  success is the destination cleared (preserving `.converge`/`.git`)
  and the entries moved in. A failed materialize — superposition,
  missing object, integrity error — leaves the workspace byte-for-byte
  untouched. `restore_snap` and both `materialize_*_to` route through
  it; the old unconditional clear is gone
- materialize now treats manifest names as untrusted: single normal
  path component only (no `..`, separators, NUL, or reserved
  `.converge`/`.git`), duplicate names within a manifest refused, and
  symlink targets refused if absolute or climbing above the
  materialized root (depth-tracked)

## Validation

`effigy validate` green (109 tests, incl. the new `safe_restore`
suite); `effigy qa:docs` green. Spec 011 archived.

## Next

Batch card 12.2: GC pending/pin mechanism replaces the mtime-grace
guess; `put_snap` verifies its root manifest; `set_lane_head` verifies
tree presence.
