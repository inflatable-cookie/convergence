# 014 Architecture Honesty

Status: in progress (14.1, 14.3 complete; 14.2 deferred)
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

- **14.1 Doc 14 reconciliation** (complete, card 050): doc 14 §0 states
  the shipped single-process server, per-section `**Deferred**` markers
  name each gap's owner, and §7 tables the unbuilt target with
  triggers; doc 16 corrected (in-request builds, edge claims); edge
  nodes and real identity stay backlog
- **14.2 Async bundle builds** — **deferred to backlog** (decision
  2026-07-25, recorded in doc 14 §7). Publish commits its publication,
  bundle, and event in one guarded batch (batch 13.1); splitting the
  build out reintroduces that interleaving window and adds worker-crash
  semantics, in exchange for latency relief on a merge already bounded
  by changed paths over a small window. Trigger to build it: a
  deployment showing publish latency actually hurting
- **14.3 Scope registry** (complete, card 051): scopes declared per
  repo with a `default` registered at repo creation; every operation
  naming an unregistered scope is refused before touching state; grant
  patterns implemented as `*` / literal / `prefix/*`
- **14.4 Operational hygiene**: event retention (prune beyond a
  configured horizon with cursor-gap signalling), GC moved off the
  request thread, per-repo GC marking where the shared store allows

## Exit Criteria

- every claim in docs 14/16/17 is either implemented or explicitly
  marked deferred; `effigy qa:docs` clean
- ~~publish returns before merge completes; bundle status observable
  through the event feed~~ — dropped with the 14.2 deferral; the
  honesty requirement it served is met by doc 14 §5 stating that builds
  are synchronous and `Building` is never constructed
- unknown scope refused; events table bounded under sustained load

## Next Task

Open batch card 14.4 (operational hygiene).
