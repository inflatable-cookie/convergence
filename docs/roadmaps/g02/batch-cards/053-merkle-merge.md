# 053 Merkle Merge

Status: complete
Updated: 2026-07-25
Roadmap: `g02.015`

## Objective

Audit 2.1 and 2.2 closed: make doc 17 §2's promise — "merge cost is
bounded by *changed* paths, with Merkle short-circuit" — actually true.
Today `merge_window` fully flattens W plus every input's base and tree
(1 + 2×window full walks per publish), folds in a flat map, then
rebuilds every manifest in the tree. A one-file publish against a 50k
path tree costs the whole tree, repeatedly.

## In Scope

- **Sparse deltas**: each input's opinions come from a recursive diff of
  its declared base against its tree that returns immediately when two
  subtree ids are equal — an untouched subtree contributes nothing and
  is never read
- **Targeted lookups**: supersession and the W-value checks need the
  value at specific paths, not whole maps; a path walk down the
  manifest chain replaces the flattened base/W maps
- **Structural reuse on write**: the merged tree is produced by
  rewriting only the manifests on paths that changed; untouched
  subtrees keep their existing manifest ids, so nothing is re-hashed or
  re-stored for an unchanged directory
- **Superposition flag from the fold** (audit 2.2): the fold already
  knows what it wrote, so the second full walk in
  `manifest_has_superpositions` goes away. W is superposition-free by
  construction (promote refuses a non-promotable bundle), and an input
  tree carrying a superposition is caught as a written value
- behavior parity is the bar: the doc 17 §2 decision table, its nine
  regression cells (batch 13.3), base-aware merge, and gate strategies
  all pass unchanged
- a test proving reuse structurally: a one-file change against a wide,
  deep tree leaves sibling subtree manifest ids identical to W's

## Out Of Scope

- pagination (15.2), TUI refresh (15.3), benchmarks (15.4 — this card
  proves reuse by structure, not by timing)
- incremental fold across successive publishes (window grew by one):
  worth its own card once the sparse fold lands

## Acceptance Criteria

- merge reads and writes only changed subtrees, proven by a test that
  asserts unchanged sibling manifest ids are reused and by object-store
  read counting; all existing merge suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `merge_window` rebuilt around three sparse operations: `diff_trees`
  (returns immediately on equal subtree ids), `lookup_path` (walks only
  the contested path for W and supersession values), and
  `apply_changes` (rewrites only manifests on changed paths, reusing
  every untouched subtree id). `flatten` / `build_tree` deleted
- `merge_window_outcome` returns `MergeOutcome { root,
  has_superpositions }`; the engine's second full walk
  (`manifest_has_superpositions`) is gone (audit 2.2). W is
  superposition-free by construction — promote refuses a
  non-promotable bundle — so the fold's own answer is complete, and a
  superposition arriving inside an input tree is caught as a written
  value
- behavior parity held with no test changes: the nine decision-table
  cells, base-aware merge, and gate strategies all pass as written
- measured: a one-file edit costs **9 manifest reads on both a 5- and a
  50-directory tree**; a publish whose tree equals its base reads **1**.
  Untouched sibling directories keep W's exact manifest ids
- doc 17 §2 now states how the promise holds across read, write, and
  classification, with the measured numbers
- 146 tests green

## Next Task

Batch card 15.2 (pagination).
