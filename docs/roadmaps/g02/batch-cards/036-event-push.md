# 036 Event Push

Status: complete
Updated: 2026-07-24
Roadmap: `g02.010`
Spec: `docs/specs/010-scale-and-transport.md`

## Objective

The server tells clients what changed; the TUI stops polling for remote
state.

## In Scope

- doc 14 amendment (short): event stream posture — per-repo SSE feed,
  at-most-once, cursor = event seq; clients reconcile via inbox on
  reconnect (events are hints, never the source of truth)
- server: in-process event bus (broadcast channel); engine emits
  `bundle` (built/status), `lane` (head moved), `release` events;
  `GET /api/repos/:repo/events?since=<seq>` as SSE (axum); events also
  queryable as JSON (`?poll=true`) for clients without streaming
- client: `events_poll(repo, since)` (poll variant; SSE consumption is
  TUI-side later — poll suffices for the exit criteria)
- TUI: background worker polls events with the cursor every few seconds
  when a remote is configured; new events refresh status/inbox data and
  surface a Last-strip note — replacing blind refresh of remote state
- tests: events emitted for publish/lane-push/release with increasing
  seq; since-cursor filtering; TUI reducer note handling

## Out Of Scope

- external backends (10.4); true SSE consumption in the TUI (polling the
  event feed already removes blind refresh)

## Acceptance Criteria

- events flow for the three flows; cursor filtering works; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- event semantics ambiguity — doc 14 first

## Outcome

- doc 14 §5b amended first: events are hints with a seq cursor; missed
  events cost freshness, never correctness; SSE deferred (poll is the
  v1 transport — contract unchanged when SSE lands)
- durable `events` table; engine emits `bundle`/`lane`/`release` on the
  three flows; `GET /api/repos/:repo/events?since=` (read capability);
  client `events()`; CLI `converge events [--since]`
- TUI: background poller (3s) with a cursor replaces blind remote
  refresh — new events surface a Last-strip note and trigger a normal
  refresh through the worker channel
- tests: three event kinds with strictly increasing seq, cursor
  filtering incl. empty tail; 95 workspace tests green

## Next Task

Execute the Batch 10.4 external-backends card
(`037-external-backends.md`).
