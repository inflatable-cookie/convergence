# 2026-07-25 Batch 15.2 Complete — Pagination

Audit 4.4 / L6 closed; card 054.

## Scoping note

The finding listed lanes, releases, bundles, publications, events, and
the inbox. In the actual HTTP surface only lanes, scopes, releases, and
the inbox are exposed: events were already paged with a continuing
cursor (batch 11.4) and gained floor/gap signalling in 14.4, and
bundles and publications have no list endpoint — they are reached
through the inbox. So the batch is three cursor listings plus the
inbox's own cost, not six endpoints.

## What landed

- `Page<T> { items, next_cursor }` on lanes, scopes, and releases, with
  `?after=&limit=`, ordered by a stable key (lane id, scope id, release
  seq) so a cursor cannot skip or repeat when rows are inserted
  concurrently
- `limit` is clamped to 1000 server-side whether or not the client
  sends it, so an unbounded response is not reachable even from a
  client that knows nothing about paging. A raw-request test pins that
- a page that fills exactly still reports a cursor — the server does
  not spend a second query proving the listing ended, so a follower
  learns it from the next short page. This is the one part of the
  contract that surprises, so doc 16 §1e states it explicitly
- **inbox**: `latest_bundles_per_gate` replaces the full-scope scan
  that read every bundle ever built to answer a question about a
  handful of gates. Lanes and publications sections are capped at 200
  with a `truncated` flag, so a large repo gets a bounded report that
  admits what it left out
- the client keeps `list_lanes` / `list_scopes` / `list_releases`
  working by following pages internally — no caller changed — and adds
  `*_page` variants for callers that want the cursor

## Validation

- `effigy validate` green: 151 tests; `effigy qa:docs` green
- new `pagination` suite: cursor walks 25 lanes in 3 pages with no gaps
  or repeats, a no-limit raw request still gets a capped page and an
  absurd limit is clamped, convenience listings return everything, scope
  cursors page in key order, and `latest_bundles_per_gate` returns 2
  rows where the scope holds 100 bundles
- conformance gained cursor-order coverage on both backends

## Next Task

Open batch card 15.3 (TUI refresh economics): a long-lived
client/workspace handle in the TUI runtime, refresh reusing it, and no
full rescan when the workspace is unchanged.
