# 011 Server Trust Boundaries

Status: in progress (11.1 complete)
Owner: repo maintainers
Updated: 2026-07-24

## Context

The 2026-07-24 audit found the server's read side never finished its
authorization story. Six endpoints authenticate but skip `authorize()`,
and the shared content-addressed object store means any read grant on one
repo discloses every private repo's content by hash. Lane namespaces are
squattable, `verify` writes to the live store from a GET, and transport
has no request-size discipline. These are release blockers; nothing else
in the audit program matters while they stand.

## Findings Addressed

- C1: `get_object`, `get_batch`, `negotiate`, `get_bundle`,
  `get_provenance`, `verify_bundle` perform no authorization —
  cross-repo disclosure through the shared object store
- C3: `personal/<subject>` lane namespace is not reserved; squatting
  hijacks default publishes and locks victims out of their own lane
- H3: `verify` mutates the shared object store as a GET side effect
- M2: `get_batch`/`put_batch` accept unbounded frame sets — memory
  amplification; no `DefaultBodyLimit` configured
- L2: `add_lane_member` bypasses the `authorize` discipline
- L4: raw `anyhow` chains leak internals cross-repo
- 1.4 (arch): `snap-sync` capability specified in doc 14 §4 but absent
  from the `Capability` enum; snap upload and lane-head push gate on
  `Publish` instead

## Execution Plan (batch details in cards)

- **11.1 Read authorization** (complete, card 038): repo-scoped object
  and negotiate routes with an object→repo association recorded on
  every server-side write; bundle-id-keyed reads resolve the bundle's
  repo and require `read` there; doc 16 §1d records the contract
- **11.2 Namespace and capability integrity**: reserve `personal/*`
  server-side; add `snap-sync` capability and regate snap upload +
  lane-head push; bring `add_lane_member` inside `authorize`
- **11.3 Read-only means read-only**: `verify` merges into a throwaway
  in-memory object store; error responses carry stable public messages
  with internals logged server-side only
- **11.4 Transport discipline**: body limits, frame-count and cumulative
  byte caps on batch endpoints, bounded `list_events` responses

## Exit Criteria

- every route either calls `authorize()` or has a documented
  reachability check; a cross-repo disclosure regression test proves a
  read grant on repo A cannot fetch repo B content
- `personal/<other-subject>` lane creation refused; squatting test
- `verify` leaves the object store byte-identical; asserted by test
- oversized batch requests rejected with a clear error

## Next Task

Open batch card 11.2 (namespace and capability integrity).
