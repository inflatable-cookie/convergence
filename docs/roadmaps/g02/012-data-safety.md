# 012 Data Safety

Status: in progress (12.1-12.2 complete)
Owner: repo maintainers
Updated: 2026-07-24

## Context

The audit found paths where Convergence destroys or silently corrupts
user data: restore clears the workspace before knowing the target is
materializable, materialize trusts manifest entry names from the wire,
GC's mtime grace window is a guess rather than a guarantee, and several
sync/interop paths turn transient failures into permanent gaps. A VCS
that can lose work under normal operation has no product; this roadmap
closes every audited loss path.

## Findings Addressed

- D1: `restore_snap` clears the tree before materializing; superposed or
  partially-fetchable targets leave an emptied workspace
- D2: `materialize_manifest` joins untrusted `entry.name`/symlink
  targets with no validation — path traversal from a hostile server
- C2 (server): GC collects uploaded-but-unpublished objects older than
  the 300s grace window; publish checks only the root manifest
- M4 (server): `put_snap` accepts records whose root manifest was never
  uploaded — dangling lane heads
- C4: `upload_tree` prunes on "server has manifest ⇒ has subtree" and
  uploads negotiate-ordered manifests unsorted — interrupted uploads
  permanently orphan subtrees
- C5: `pull_lane` swallows transient errors as thinned gaps — truncated
  lineage presented as authoritative
- G2: git-map written non-atomically and after the ref move — crash
  yields duplicate commits and permanent divergence
- D3: torn snapshots under concurrent writes (silent small-file
  truncation vs hard large-file abort)
- R1: `write_atomic` never fsyncs — power loss can zero state files
- R2: `read_config` writes as a side effect on the hot path

## Execution Plan (batch details in cards)

- **12.1 Safe restore** (complete, card 042): `materialize_via_temp`
  defers destruction until the tree fully materializes; materialize
  validates entry names, symlink targets, and duplicate names
- **12.2 GC reachability guarantee** (complete, card 043): upload
  pins protect not-yet-referenced objects independent of clock time,
  released when the tree is durably referenced; `upload_snap_record`
  verifies its root manifest object is present (audit M4)
- **12.3 Honest sync failure**: `pull_lane` distinguishes 404 from
  transient errors; `upload_tree` re-sorts child-first and re-verifies
  leaf presence after interruption
- **12.4 Durability details**: atomic + fsync'd writes for git-map,
  state, config, HEAD; git-map persisted before the ref moves; capture
  re-stats files to detect mid-write tears; `read_config` made pure

## Exit Criteria

- interrupted-restore, hostile-manifest, and interrupted-upload tests
  all preserve user data or fail before destruction
- an upload → wait past grace → GC → publish sequence keeps every blob
- kill-mid-export followed by re-export produces no duplicate commits

## Next Task

Open batch card 12.3 (honest sync failure).
