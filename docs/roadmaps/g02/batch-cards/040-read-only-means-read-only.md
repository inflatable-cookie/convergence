# 040 Read-Only Means Read-Only

Status: complete
Updated: 2026-07-24
Roadmap: `g02.011`
Spec: `docs/specs/archive/011-server-trust-boundaries.md`

## Objective

Audit H3 (verify writes to the live store from a GET) and L4 (raw error
chains leak internals) closed.

## In Scope

- `ScratchObjects` overlay store: reads fall through to the shared
  store, writes stay in memory and are discarded — `verify` replays the
  merge through it, leaving the object store byte-identical
- error hygiene: read handlers that touch stores directly return a
  stable public message with the internal chain logged server-side
  (500); engine domain errors (user-actionable) keep their top-level
  message at 400
- regression test: object store contents identical before/after verify

## Out Of Scope

- structured logging framework (eprintln is slice-grade), token
  lifecycle (backlog)

## Acceptance Criteria

- verify leaves the store byte-identical under test; no handler
  response carries a nested store error chain; suites green

## Validation

- `effigy validate`

## Outcome

- `ScratchObjects` copy-on-write overlay (reads fall through, writes
  stay in memory); `verify` replays through it — regression test proves
  the store is byte-identical before/after a verify of a
  text-line-merge bundle
- error hygiene: direct store reads in handlers return 500 with a
  stable "internal error" message and the chain logged server-side;
  engine domain errors (user-actionable) keep top-level messages at 400

## Next Task

Batch card 11.4 (transport discipline).
