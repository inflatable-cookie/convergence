# 2026-07-25 Batch 15.1 Complete — Merkle Merge

Audit 2.1 and 2.2 closed; card 053, roadmap `g02.015` opened. Doc 17
§2's promise — "merge cost bounded by changed paths" — was false; it is
now true and measured.

## What was wrong

`merge_window` flattened W plus every input's base and tree into full
path maps (1 + 2×window complete tree walks per publish), folded in the
flat map, then rebuilt every manifest in the tree from scratch. A
one-file publish against a large tree cost the entire tree, on every
publish in the window. The engine then walked the merged tree a second
time just to ask whether it contained superpositions.

## What landed

Three sparse operations replace the flatten-everything approach:

- `diff_trees` — per-input delta that returns immediately when two
  subtree ids are equal, so an untouched directory is never opened.
  Handles leaf↔directory type changes by emitting the deletes and sets
  the old flatten-based comparison produced
- `lookup_path` — the fold needs W's value and other inputs' base
  values only at contested paths, so those walk down the specific path
  instead of flattening a tree
- `apply_changes` — rewrites only the manifests along changed paths.
  Untouched subtrees keep their existing manifest ids, so nothing is
  re-hashed or re-stored for a directory nobody edited. A directory
  emptied by deletions disappears rather than lingering

`merge_window_outcome` returns `MergeOutcome { root, has_superpositions
}`, and the engine's second walk is deleted (audit 2.2). W is
superposition-free by construction — promote refuses a non-promotable
bundle — so the fold's own answer is complete; a superposition arriving
inside an input tree is caught as a written value.

## Validation

- `effigy validate` green: 146 tests; `effigy qa:docs` green
- **Behavior parity with no test edits**: the nine decision-table cells
  (batch 13.3), base-aware merge, and gate strategies all pass exactly
  as written. That existing coverage is what made this rewrite safe
- new `merkle_merge` suite measures the property:
  - a one-file edit costs **9 manifest reads against both a 5-directory
    and a 50-directory tree** — flatten-everything would have scaled
    with tree size
  - a publish whose tree equals its base reads **1** manifest
  - untouched sibling directories in the merged tree carry W's exact
    manifest ids
- doc 17 §2 now explains how the promise holds across read, write, and
  classification, with those numbers

## Next Task

Open batch card 15.2 (pagination): cursor + limit on every list
endpoint and the inbox, client/TUI paging, wire DTO changes in doc 16.
