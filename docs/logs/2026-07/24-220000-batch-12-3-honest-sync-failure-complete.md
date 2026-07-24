# 2026-07-24 Batch 12.3 Complete — Honest Sync Failure

Audit C5 (`pull_lane` swallows transient errors as thinned gaps) and C4
(`upload_tree` uploads negotiate-ordered manifests unsorted and prunes
leaf collection on "server has manifest ⇒ has subtree") are closed;
card 044, roadmap `g02.012`.

## What landed

- `pull_lane` inspects the snap-record status: only a 404 is a thinned
  gap (walk stops there); 5xx, auth, and transport failures fail the
  pull instead of presenting a truncated lineage as authoritative
- `upload_tree` streams missing manifests child-first — the reverse of
  the local parent-first collect order — so an interrupted batch stream
  can never leave a parent manifest on the server without its children
- `upload_tree` collects blob/recipe candidates from every reachable
  manifest, not only the missing ones, and negotiates them: leaf holes
  under manifests the server already has (a previously torn upload)
  are detected and re-uploaded. Manifest holes already healed, since
  the full manifest list is negotiated each time

## Validation

- `effigy validate` green: fmt, clippy, 113 tests passed
- new `e2e_sync` coverage: torn-state heal (leaf blob + child manifest
  deleted server-side, re-upload heals, fresh fetch materializes),
  thinned ancestor still pulls, stub server returning 500 mid-walk
  fails the pull with the status in the error

## Next Task

Open batch card 12.4 (durability details): fsync'd atomic writes
(git-map, state, config, HEAD), git-map persisted before the ref
moves, capture re-stat guard, pure `read_config`.
