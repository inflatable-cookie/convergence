# Spec 012: Data Safety

Status: active
Roadmap: `g02.012`
Updated: 2026-07-24

## Intent

Close every audited path where Convergence destroys or silently
corrupts user data (audit D1-D3, D5, C4, C5, G2, G4, R1, R2, server C2,
M4). A VCS that can lose work under normal operation has no product.

## Execution grammar

Four batches, sequenced:

1. **12.1 Safe restore** — materialize into a temp dir inside the
   destination, then swap; hostile-manifest validation (entry names,
   symlink targets, duplicates).
2. **12.2 GC reachability guarantee** — pending/pin mechanism replaces
   the mtime-grace guess; `put_snap` verifies its root manifest;
   `set_lane_head` verifies tree presence.
3. **12.3 Honest sync failure** — `pull_lane` distinguishes absent from
   transient; `upload_tree` child-first ordering + leaf re-verification.
4. **12.4 Durability details** — fsync'd atomic writes (git-map, state,
   config, HEAD), git-map persisted before the ref moves, capture
   re-stat guard, pure `read_config`.

## Design pins

- The workspace is never cleared before the replacement tree fully
  materialized on the same filesystem; a failed materialize leaves the
  workspace untouched.
- Manifest entry names are single normal path components; `.converge`
  and `.git` are refused; duplicate names in one manifest are refused
  (collision ordering attacks). Symlink targets may not be absolute and
  may not escape the materialized root by `..` depth.
- Validation lives with materialize and is shared by restore, fetch
  `--into`, and git export worktrees — anything that turns manifests
  into filesystem trees.

## Exit

Roadmap `g02.012` exit criteria: interrupted-restore, hostile-manifest,
interrupted-upload, GC-race, and kill-mid-export scenarios all preserve
user data or fail before destruction, proven by tests.
