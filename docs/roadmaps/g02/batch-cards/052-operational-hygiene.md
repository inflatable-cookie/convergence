# 052 Operational Hygiene

Status: complete
Updated: 2026-07-25
Roadmap: `g02.014`

## Objective

Close the roadmap's operational findings: the events table grows
forever with nothing to prune it, and GC blocks a runtime worker for
the length of a whole-store walk. Both are single-node problems that
bite the beachhead, not scale aspirations.

## In Scope

- **Event retention**: `RetentionPolicy` gains `keep_events`; GC prunes
  a repo's events beyond the newest N and records the pruned high-water
  mark as the repo's **event floor**
- **Cursor-gap signalling**: `GET .../events` returns a body carrying
  `events`, the `floor`, and `gap: bool` — true when the caller's
  cursor sits below the floor, meaning events were pruned that they
  never saw. A missed event costs freshness, not correctness (doc 14
  §5b), but a *silent* gap lets a client believe it is caught up when
  it is not. Client surfaces the flag; the wire shape changes (pre-1.0,
  no shim)
- **GC off the runtime**: `run_gc` moves its work into
  `spawn_blocking`, so a whole-store walk stops stalling other
  requests' futures on a shared executor thread; plus a single-flight
  guard so a second concurrent GC is refused with a clear error rather
  than duplicating a full walk
- **Per-repo marking**: resolved as *not possible*, and recorded as
  such rather than left as an open intention — the object store is
  deduplicated across repos, so marking only the triggering repo's
  roots would sweep another repo's live content. Doc 14 §2 already
  states this; this batch adds the test that pins the behavior

## Out Of Scope

- async bundle builds (deferred, doc 14 §7); moving publish's merge off
  the runtime thread (same `spawn_blocking` shape, but publish is short
  by construction — note it, do not build it)
- a job registry or GC scheduling; GC stays admin-triggered

## Acceptance Criteria

- events prune to the configured horizon; a stale cursor reports
  `gap: true` and a fresh one does not; two concurrent GCs do not both
  walk; cross-repo objects survive a neighbouring repo's GC; all suites
  green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `RetentionPolicy::keep_events`; GC prunes beyond the newest N and
  records the highest pruned seq in `event_floors` as the repo's floor.
  Pruning is idempotent and the floor never rewinds; dry runs prune
  nothing
- the feed returns `EventPage { events, floor, gap }`; `gap` is true
  when the cursor sits below the floor. Client gains `event_page`, with
  `events` kept as the thin wrapper for callers with a fresh cursor
- `run_gc` moved into `spawn_blocking` — a whole-store walk no longer
  stalls other requests' futures on a runtime worker — plus a
  `gc_running` single-flight guard that refuses a concurrent run rather
  than repeating the walk
- per-repo marking resolved as **not possible**, not deferred: the
  object store is deduplicated across repos, so narrowing the mark
  would sweep a neighbour's live content. A test now pins that a GC in
  one repo leaves another repo's reachable objects alone
- doc 14 §2 and §5b updated; §7 drops the retention/GC row and notes
  that publish's merge would take the same `spawn_blocking` fix before
  anyone reaches for async workers
- 143 tests green

## Next Task

Close roadmap `g02.014`; open `g02.015` (scale walls).
