# 015 Scale Walls

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

## Context

The audit quantified where the system hits walls, in order: full-window
remerge with no Merkle pruning (quadratic per window, dominates on the
binary-heavy beachhead), the global write lock, full-store GC scans,
unpaginated list endpoints, and a TUI that rebuilds the entire client
stack every three seconds. Doc 17 §2 promises "merge cost bounded by
changed paths" — currently false. This roadmap makes the promise true
and removes the unbounded surfaces.

## Findings Addressed

- 2.1 (arch): `merge_window` flattens every manifest fully — no
  subtree-hash short-circuit; O(window × full tree) per publish
- 2.2: `manifest_has_superpositions` re-walks the merged tree per
  publish
- 4.4 (arch) / L6 (server): no pagination on lanes, releases, bundles,
  publications, events, inbox; inbox is O(gates × window + bundles)
  per call
- 2.5 (arch): TUI refresh re-parses argv and rebuilds store/workspace
  per tick and per keystroke command
- GC cost = total live objects across the deployment per run

## Execution Plan (batch details in cards)

- **15.1 Merkle merge**: identical-subtree pruning in `merge_window`
  (doc 17 §2 made true); superposition flag computed during the merge
  fold instead of a second walk; incremental fold onto the previous
  bundle where the window grew by one
- **15.2 Pagination**: cursor + limit on every list endpoint and the
  inbox; client/TUI paging support; wire DTO changes in doc 16
- **15.3 TUI refresh economics**: long-lived client/workspace handle in
  the TUI runtime; refresh reuses it and skips rescan when the
  workspace is unchanged (mtime/dirstamp check); event-driven refresh
  includes inbox
- **15.4 Scale proof**: large-tree and large-window benchmarks in CI
  (ignored-by-default like the CBOR benchmark) demonstrating merge cost
  tracks changed paths, not tree size

## Exit Criteria

- publish cost measured proportional to changed paths on a 50k-path
  tree benchmark
- no endpoint returns an unbounded result set
- TUI idle refresh does no full workspace rescan

## Next Task

Blocked behind g02.014 completion.
