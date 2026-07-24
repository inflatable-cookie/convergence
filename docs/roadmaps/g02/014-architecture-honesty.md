# 014 Architecture Honesty

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

## Context

Doc 14 describes an async, partitioned, edge-cached data plane; the
code is a single-process synchronous server behind one global lock.
This is the g01 failure mode recurring in milder form: documentation
claiming distributed-scale properties the implementation does not
have. The remedy is a deliberate reconciliation — implement the parts
that matter now, downgrade the rest to an explicit deferred design —
so the docs are true again.

## Findings Addressed

- 1.1 (arch): bundle builds run synchronously inside the publish
  request; `Building` status never constructed; doc 14 §5 and doc 16
  §1 claim async coalescing
- 1.2: both backends are a global single-writer mutex; doc 14 §1/§3
  claim partitioned no-global-locks writes
- 1.3: edge nodes entirely unbuilt
- 1.5: tokens are a static startup map; doc 14 §4 claims short-lived
  capability-scoped tokens
- 1.6: GC is a global cross-repo scan; doc 14 §2 claims
  partition-scoped, never stop-the-world
- 2.4 / M3: no scope registry — free-string `scope_id` mints unbounded
  partitions and fragments windows; `scope_pattern` grants are literal
  equality only
- Events table grows forever; nothing prunes it

## Execution Plan (batch details in cards)

- **14.1 Doc 14 reconciliation**: rewrite doc 14 to describe the
  single-node synchronous slice as current state, moving the async /
  partitioned / edge design into an explicit "target architecture"
  section with honest gap markers; align doc 16 §1 and doc 17 §2
  claims; edge nodes and real identity stay backlog
- **14.2 Async bundle builds**: publish enqueues; a build worker
  produces bundles through the `building → ready/failed` status
  machine; clients observe status via events
- **14.3 Scope registry**: scopes declared per repo (auto-provision
  policy decided in the card); publish/fetch validate scope existence;
  grant patterns either implemented or renamed to literal
- **14.4 Operational hygiene**: event retention (prune beyond a
  configured horizon with cursor-gap signalling), GC moved off the
  request thread, per-repo GC marking where the shared store allows

## Exit Criteria

- every claim in docs 14/16/17 is either implemented or explicitly
  marked deferred; `effigy qa:docs` clean
- publish returns before merge completes; bundle status observable
  through the event feed
- unknown scope refused; events table bounded under sustained load

## Next Task

Blocked behind g02.013 completion.
