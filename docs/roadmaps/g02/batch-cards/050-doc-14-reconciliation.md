# 050 Doc 14 Reconciliation

Status: complete
Updated: 2026-07-25
Roadmap: `g02.014`

## Objective

Audit findings 1.1, 1.2, 1.3, 1.5, 1.6 and the doc side of 2.4/M3
closed: doc 14 states what the server actually is — a single-process
synchronous slice — and moves every unbuilt distributed property into
an explicitly deferred target section. The g01 failure mode was docs
claiming scale properties the code lacked; this batch makes the docs
true before later batches close the gaps for real.

## In Scope

- doc 14 gains a status legend and per-claim honesty: present tense
  describes shipped behavior only, everything else is marked deferred
  with the roadmap or backlog that owns it
- corrections: data plane is one process with a mutex-guarded metadata
  connection (not horizontally scaled, no partition workers); GC marks
  across every repo (not partition-scoped); tokens are a static startup
  map (not short-lived capability-scoped); edge nodes do not exist;
  bundle builds are synchronous inside publish (`Building` is never
  constructed today)
- known gaps recorded where they belong: no scope registry (free-string
  `scope_id`, `scope_pattern` is literal equality), unbounded events
  table — each pointing at its owning batch
- new `## 7. Target architecture (deferred)` collecting edges, async
  partition workers, real identity, and horizontal partitioning, each
  with why-not-yet and what would trigger it
- doc 16 §1 step 3 corrected: the server builds the bundle in-request
  rather than enqueuing coalescing
- `docs/architecture/README.md` reflects the reconciliation if it
  summarizes doc 14 claims

## Out Of Scope

- implementing async builds (14.2), the scope registry (14.3), or
  event retention and GC scoping (14.4) — this batch only tells the
  truth about them
- doc 14 §3 consistency model (already corrected in batch 13.2)

## Acceptance Criteria

- no present-tense claim in docs 14/16 describes unbuilt behavior;
  every deferred property names its owner; `effigy qa:docs` clean and
  the wider suite unaffected

## Validation

- `effigy qa:docs`
- `effigy validate`

## Outcome

- doc 14 gained `## 0. What is built, and what is not`: the reading rule
  (present tense = shipped), a one-paragraph statement of the real
  server (one process, one mutex-guarded metadata connection, inline
  merge, no edges/workers/scaling), and the list of what genuinely
  ships — partition key, guarded writes, enforced authz, deterministic
  merge, backend seam
- per-section corrections, each marked `**Deferred**` with its owner:
  §1 partitioning buys no parallelism yet and `scope_id` is
  unvalidated; §1 edge nodes do not exist; §2 GC marks across every
  repo (the object store is shared, so a narrower mark would sweep
  another repo's live content) and runs inline; §3 control plane is
  serialized globally, which is strictly stronger than the claim; §4
  authentication is a static token map while authorization is fully
  enforced, and grants are literal-or-`*`; §5 builds are synchronous
  and `Building` is never constructed; §5b events are unbounded
- new `## 7. Target architecture (deferred)` table: six unbuilt
  properties, each with current state and the roadmap batch or backlog
  trigger that owns it, plus a note on which shipped seams make them
  additive rather than rewrites
- §6 split into current posture (single process is the availability
  unit and write ceiling — acceptable for the beachhead, measured by
  the scale-walls roadmap) and target posture
- doc 16: step 3 builds in-request rather than enqueuing; the two
  present-tense edge claims corrected; architecture README points at
  §0/§7

## Next Task

Batch card 14.2 (async bundle builds).
