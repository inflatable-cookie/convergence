# 2026-07-25 Batch 14.1 Complete — Doc 14 Reconciliation

Audit findings 1.1, 1.2, 1.3, 1.5, 1.6 and the doc side of 2.4/M3 are
closed as documentation defects; card 050, roadmap `g02.014` opened.
The code gaps stay open — later batches close them — but the docs no
longer claim they are shut.

## What landed

- doc 14 `## 0. What is built, and what is not`: the reading rule
  (present tense means shipped; anything else is marked `**Deferred**`),
  a plain statement of the real server — one binary, one process, both
  planes on a single mutex-guarded metadata connection, merge inline in
  the publish request, no edges, no workers, no horizontal scaling —
  and the list of what genuinely ships: partition key, guarded
  transactional writes, enforced authz, deterministic merge, backend
  seam
- per-section corrections, each naming the batch or backlog that owns
  the gap:
  - §1 the partition key is real but buys no parallelism yet;
    `scope_id` is an unvalidated free string (14.3)
  - §1 edge nodes: no edge code exists; every edge mention is target
  - §2 GC marks across every repo and sweeps the whole store — the
    object store is shared and deduplicated, so a narrower mark would
    sweep another repo's live content — and runs on the request thread
    (14.4)
  - §3 control plane is serialized globally, strictly stronger than the
    per-repo claim
  - §4 authorization is fully enforced; *authentication* is a static
    startup token map with no expiry, capabilities, or revocation.
    Grants are literal-or-`*`, no globbing
  - §5 builds are synchronous; `BundleStatus::Building` exists in the
    model and is never constructed (14.2)
  - §5b the events table is append-only and unbounded (14.4)
- new `## 7. Target architecture (deferred)`: six unbuilt properties in
  a table with current state and the trigger that would build each —
  async workers, scope registry, event retention/GC scoping to their
  roadmap batches; horizontal scaling to a measured write ceiling;
  edges to a real multi-site customer; short-lived tokens to any
  deployment outside a trusted network
- §6 split: current posture states the single process is the
  availability unit and the write ceiling, acceptable for the beachhead
  and measured rather than assumed by the scale-walls roadmap; the
  previous claims move to target posture
- doc 16: commit-intent step builds in-request instead of enqueuing
  coalescing; the two present-tense edge claims corrected; architecture
  README points at §0/§7

## Validation

- `effigy qa:docs` green; `effigy validate` green (134 tests, no code
  change in this batch)

## Next Task

Open batch card 14.2 (async bundle builds): publish enqueues, a build
worker drives `building → ready/failed`, clients observe status through
the event feed.
