# 2026-07-25 Batch 14.4 Complete — Operational Hygiene, g02.014 Closed

Card 052. Roadmap `g02.014` is complete: 14.1, 14.3, and 14.4 shipped;
14.2 deferred to backlog with a recorded trigger.

## What landed

- **Event retention**: `RetentionPolicy::keep_events` bounds the table.
  GC prunes everything beyond the newest N and records the highest
  pruned seq in `event_floors` as the repo's floor. Pruning is
  idempotent, the floor never rewinds, and a dry run prunes nothing
- **Cursor-gap signalling**: the feed now returns
  `EventPage { events, floor, gap }`. `gap` is true when the caller's
  cursor sits below the floor, meaning pruned events it never saw.
  Events are hints, so the cost is freshness — but a client must never
  believe a truncated page was complete. Client gains `event_page`;
  `events` stays as the thin wrapper for callers with a fresh cursor
- **GC off the runtime**: `run_gc` moved into `spawn_blocking`, so a
  whole-store walk no longer stalls other requests' futures on a shared
  runtime worker, plus a `gc_running` single-flight guard that refuses
  a concurrent run rather than repeating the walk
- **Per-repo marking resolved, not deferred**: the roadmap asked for
  per-repo GC marking "where the shared store allows". It does not
  allow it — the object store is deduplicated across repos, so marking
  only the triggering repo's roots would sweep a neighbour's live
  content. Global marking is the correct consequence of dedup; a test
  now pins that a GC in one repo leaves another repo's reachable
  objects intact

## Validation

- `effigy validate` green: 143 tests; `effigy qa:docs` green
- new `operational_hygiene` suite: prune-to-horizon with floor,
  dry-run prunes nothing, per-repo floors, cross-repo survival;
  `transport_and_verify` gained the HTTP-level stale-cursor gap test;
  conformance covers `prune_events` / `event_floor` on both backends

## Roadmap g02.014 exit criteria

- docs 14/16/17 claims implemented or explicitly deferred (14.1)
- unknown scope refused (14.3)
- events bounded, stale cursors told about the gap (14.4)
- the async-publish criterion was struck with the 14.2 deferral; the
  honesty requirement behind it is met by doc 14 §5

## Next Task

Open roadmap `g02.015` (scale walls), batch card 15.1. Note that doc 14
§7 now names the scale-walls roadmap as the trigger for horizontal
scaling and (indirectly) for async builds — measuring the walls is what
tells us whether either deferral should end.
