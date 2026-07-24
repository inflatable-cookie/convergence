# 015 Scale Walls

Status: in progress (15.1-15.3 complete)
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

- **15.1 Merkle merge** (complete, card 053): sparse diff with
  identical-subtree pruning, path-walk lookups, and structural reuse on
  write (doc 17 §2 made true); superposition flag from the fold instead
  of a second walk. Incremental fold across successive publishes was
  split out as its own follow-up rather than bundled here
- **15.2 Pagination** (complete, card 054): cursor + server-clamped
  limit on lanes, scopes, and releases (events were already paged); the
  inbox capped with a `truncated` flag and reading one bundle per gate
  instead of scanning the scope; client follows pages internally; doc
  16 §1e carries the contract
- **15.3 TUI refresh economics** (complete, card 055):
  `converge_cli::Session` holds the workspace handle, the working-tree
  scan, and the remote connection pool across commands; the scan is
  keyed by a metadata-only dirstamp so an idle refresh stats instead of
  hashing. The TUI holds one session for its lifetime and refreshes the
  inbox on event arrival. Capture paths (`snap`, `watch`) never read the
  cache — the stamp's same-tick blind spot must not reach a snapshot
- **15.4 Scale proof**: large-tree and large-window benchmarks in CI
  (ignored-by-default like the CBOR benchmark) demonstrating merge cost
  tracks changed paths, not tree size

## Exit Criteria

- publish cost measured proportional to changed paths on a 50k-path
  tree benchmark
- no endpoint returns an unbounded result set
- TUI idle refresh does no full workspace rescan

## Next Task

Open batch card 15.4 (scale proof: large-tree and large-window
benchmarks in CI, ignored by default).
