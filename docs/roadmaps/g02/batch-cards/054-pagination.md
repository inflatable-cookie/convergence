# 054 Pagination

Status: complete
Updated: 2026-07-25
Roadmap: `g02.015`

## Objective

Audit 4.4 / L6 closed: no endpoint returns an unbounded result set, and
the inbox stops scanning every bundle in a scope to answer a question
about a handful of gates.

## Scope of the actual problem

The exposed list surfaces are lanes, scopes, releases, and the inbox.
Events were already paged with a continuing cursor (batch 11.4) and
gained floor/gap signalling in 14.4, so they need nothing. Bundles and
publications have no list endpoint — they are reached through the
inbox — so the fix there is the inbox's own cost, not a public page.

## In Scope

- `Page<T> { items, next_cursor }` as the response shape for lanes,
  scopes, and releases; `?after=<cursor>&limit=<n>` on each, ordered by
  a stable key (lane id, scope id, release seq) so a cursor cannot skip
  or repeat under concurrent inserts
- a server-side cap applied whether or not the client sends `limit`,
  matching the events feed's existing 1000 — an unbounded response is
  not reachable even by an old client
- **inbox cost**: replace the `list_bundles` full-scope scan with a
  targeted `latest_bundles_per_gate` query returning at most one row
  per gate; cap the lanes and publications sections and report
  `truncated` when a section was cut, so a large repo returns a bounded
  report that admits what it left out
- client methods carry the cursor; CLI list verbs follow pages to
  completion so `converge lane list` still shows everything
- doc 16 gains the paging contract (cursor semantics, cap, truncation)

## Out Of Scope

- TUI refresh economics (15.3) and benchmarks (15.4)
- paginating GC's internal walks: GC must see every root by definition
  (doc 14 §2), and its cost is bounded instead by running off the
  runtime with a single-flight guard (batch 14.4)

## Acceptance Criteria

- lanes, scopes, and releases page with a working cursor and are capped
  server-side; the inbox reads at most one bundle per gate and reports
  truncation; CLI list output is unchanged for small repos; all suites
  green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `Page<T> { items, next_cursor }` on lanes, scopes, and releases, with
  `?after=&limit=`; ordering by lane id, scope id, and release seq so a
  cursor cannot skip or repeat under concurrent inserts
- `limit` is clamped to 1000 server-side whether or not the client
  sends it — a client that knows nothing of paging still cannot pull an
  unbounded response, and the raw-request test pins that
- a page that fills exactly still reports a cursor: the server does not
  spend a second query proving the listing ended, so followers learn it
  from the next short page. Documented in doc 16 §1e because it is the
  one part of the contract that surprises
- **inbox cost**: `latest_bundles_per_gate` replaces the full-scope
  scan — one row per gate from the store instead of every bundle ever
  built. Lanes and publications sections are capped at 200 with a
  `truncated` flag so a large repo gets a bounded report that admits
  what it cut
- client keeps `list_lanes` / `list_scopes` / `list_releases` working
  by following pages internally, so no caller changed; `*_page`
  variants expose the cursor for callers that want it
- doc 16 gained §1e (paging contract) noting the event feed keeps its
  own `{events, floor, gap}` shape because it carries pruning
  information a plain page has no place for
- 151 tests green

## Next Task

Batch card 15.3 (TUI refresh economics) — done, card 055.
