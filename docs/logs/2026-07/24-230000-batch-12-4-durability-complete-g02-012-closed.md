# 2026-07-24 Batch 12.4 Complete — Durability Details, g02.012 Closed

Audit R1 (`write_atomic` never fsyncs), G2 (git-map non-atomic, saved
after the ref move), D3 (torn snapshots under concurrent writes), R2
(`read_config` writes on the hot path) are closed; card 045. All four
data-safety batches done — roadmap `g02.012` complete, spec 012
archived.

## What landed

- `write_atomic` fsyncs the temp file before the rename and the parent
  directory after; config, state, HEAD, snaps, resolutions, and the
  git-map all flow through it — power loss can neither zero a state
  file nor lose the rename
- git export commits against `refs/converge/export-tmp`, persists the
  updated map atomically, then moves `refs/heads/<branch>` and drops
  the temp ref. fast-import is deterministic (snap timestamps, fixed
  committer), so a crash anywhere re-converges to identical shas —
  no duplicate commits, no divergence
- capture reads files stat → read → re-stat (len + mtime, 3 attempts)
  in both scan paths; chunk helpers take the already-read bytes,
  removing the second racy read; a file that keeps changing fails the
  snap loudly
- `read_config` is pure: the legacy config-token migration is dropped
  (pre-1.0 posture, tokens live in state.json, `converge login`
  recovers a legacy workspace)

## Validation

- `effigy validate` green: fmt, clippy, 116 tests passed
- new coverage: lost-map re-export produces identical head sha and no
  duplicate history, temp ref cleaned up; concurrent-writer capture
  loop proves a successful snap is never torn; `read_config` leaves
  config.json byte-identical

## Roadmap g02.012 exit criteria

- interrupted-restore, hostile-manifest, interrupted-upload scenarios
  preserve data or fail before destruction (12.1, 12.3)
- upload → zero-grace GC → publish keeps every object (12.2)
- kill-mid-export then re-export yields no duplicate commits (12.4)

## Next Task

Open roadmap `g02.013` (transactional and merge correctness), batch
card 13.1.
