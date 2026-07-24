# 042 Safe Restore

Status: complete
Updated: 2026-07-24
Roadmap: `g02.012`
Spec: `docs/specs/012-data-safety.md`

## Objective

Audit D1 (restore clears the workspace before knowing the target
materializes) and D2 (path traversal from untrusted manifests) closed.

## In Scope

- `materialize_via_temp`: materialize the full tree into a temp dir
  inside the destination first; only on success clear the destination
  (preserving `.converge`/`.git`) and move the entries in; a failed
  materialize (superposition, missing object, integrity error) leaves
  the destination untouched
- `restore_snap`, `materialize_snap_to`, `materialize_manifest_to` all
  route through it
- manifest validation in materialize: entry names must be single normal
  components (no `..`, no separators, no empty, not `.converge`/`.git`);
  duplicate names in one manifest refused; symlink targets must be
  relative and may not escape the materialized root by `..` depth
- tests: superposed-target restore preserves the workspace; missing-
  blob restore preserves the workspace; traversal names and absolute /
  escaping symlink targets refused; happy-path restore still works

## Out Of Scope

- GC pinning (12.2), sync failure honesty (12.3), fsync durability
  (12.4)

## Acceptance Criteria

- hostile-manifest and interrupted-restore tests green; all existing
  suites green

## Validation

- `effigy validate`

## Outcome

- `materialize_via_temp`: the tree lands in a temp dir inside the
  destination first (same filesystem, so the swap is renames); only on
  success is the destination cleared (preserving `.converge`/`.git`)
  and the entries moved in — a failed materialize leaves the workspace
  untouched. `restore_snap` and both `materialize_*_to` route through
  it; the old unconditional `clear_workspace_except_converge_and_git`
  is gone
- materialize validates every manifest entry name (single normal
  component, no `..`/separators/NUL, not `.converge`/`.git`), refuses
  duplicate names within one manifest, and refuses symlink targets that
  are absolute or climb above the materialized root (depth-tracked)
- new `safe_restore` suite: superposed-target and missing-blob restores
  preserve the workspace with no temp debris; traversal name, escaping
  symlink, and duplicate name all refused; happy-path restore intact —
  109 workspace tests green

## Next Task

Batch card 12.2 (GC reachability guarantee).
