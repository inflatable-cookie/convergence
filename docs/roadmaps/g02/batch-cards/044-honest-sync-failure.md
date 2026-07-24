# 044 Honest Sync Failure

Status: complete
Updated: 2026-07-24
Roadmap: `g02.012`
Spec: `docs/specs/012-data-safety.md`

## Objective

Audit C5 (`pull_lane` swallows transient errors as thinned gaps) and C4
(`upload_tree` uploads negotiate-ordered manifests unsorted and prunes
leaf collection on "server has manifest ⇒ has subtree") closed: sync
failures are loud, interrupted uploads heal on retry.

## In Scope

- `pull_lane` distinguishes absent from broken: 404 on a snap record is
  a legitimate thinned gap (walk stops there); any other failure —
  transport error, 5xx, auth — fails the pull instead of presenting a
  truncated lineage as authoritative
- `upload_tree` orders missing manifests child-first (reverse of the
  local parent-first collect order), so a torn batch stream can never
  leave a parent manifest on the server before its children
- `upload_tree` collects blob/recipe candidates from every reachable
  manifest, not only the missing ones, and negotiates them — leaf holes
  from a previously interrupted upload are detected and re-uploaded
  (manifest holes already heal: the full manifest list is negotiated)
- tests: torn-state heal (delete a child manifest and a leaf blob
  server-side, re-run `upload_tree`, tree fetches complete); thinned
  gap still pulls (404 path); non-404 snap failure fails the pull

## Out Of Scope

- durability/fsync work (12.4); resumable-upload protocol or upload
  progress persistence (transport scale, not correctness)

## Acceptance Criteria

- torn server tree heals on re-upload under test; pull with a thinned
  ancestor succeeds; pull with a failing server errors; all suites
  green

## Validation

- `effigy validate`

## Outcome

- `pull_lane` treats only a 404 snap record as a thinned gap; any other
  failure (5xx, auth, transport) fails the pull — truncated lineage no
  longer presents as authoritative
- `upload_tree` streams missing manifests child-first (reverse of the
  parent-first collect order), so a torn batch stream cannot leave a
  parent manifest on the server without its children
- `upload_tree` negotiates blob/recipe leaves from every reachable
  manifest, not just missing ones — leaf holes under manifests the
  server already has heal on re-upload
- tests (`e2e_sync`): torn-state heal (leaf blob + child manifest
  deleted server-side, re-upload heals, tree materializes), thinned
  ancestor still pulls, stub server 500 mid-walk fails the pull; 113
  tests green

## Next Task

Batch card 12.4 (durability details).
