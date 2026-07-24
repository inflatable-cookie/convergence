# 14 Server Authority and Distribution

Status: active
Updated: 2026-07-25
Roadmap: `g02.002` Batch 2.4; reconciled with the implementation in
`g02.014` Batch 14.1

The server design for the rebuild. This doc owns the claim the g01 era never
honored: large distributed development organizations as the primary target.
Operator decisions (2026-07-23): **central control plane with a partitioned
data plane**, shipped as **one binary with pluggable storage backends**.

Research anchors: the Perforce dossier (centralized fragility, single-server
write ceiling, offline pain) and the Jujutsu dossier (conflicts-as-data,
operation log) — `docs/research/specimen-dossiers/`.

## 0. What is built, and what is not

The g01 failure mode was documentation claiming distributed-scale
properties the code did not have. This doc therefore separates the
**design** (which the operator decisions above still commit to) from the
**current implementation** (a single-process synchronous server).

Reading rule for everything below: **present tense describes shipped
behavior.** Anything not yet built is marked `**Deferred**` inline and
collected in §7, each with the roadmap or backlog that owns it. If you
find a claim here that the code does not honor, that is a bug in this
doc — fix the doc in the same change that discovers it.

Current implementation in one paragraph: one binary, one process. Both
planes share a single metadata store (SQLite embedded, Postgres
optional) behind one mutex-guarded connection, and one object store
(local FS embedded, S3 optional). Publishes are handled synchronously —
the bundle merge runs inside the publish request. There are no edge
nodes, no partition workers, and no horizontal scaling. What *is* real:
the partitioned data model, guarded transactional writes (§3), authz on
every data-plane operation (§4), deterministic merge (§5), and the
pluggable-backend seam (§2). Those are the parts later scale work
builds on rather than replaces.

## 1. Shape

Two planes, one binary (services enabled by role config):

- **Control plane** — the single logical authority for identity, permissions,
  repo registry, gate graphs, scope/lane registries, and provenance. Small
  data, strong consistency, low write volume. This is what "central server is
  authority" means in the vision — and it is the only part that must be
  globally consistent.
- **Data plane** — convergence state (publications, bundles, promotions,
  releases, lane heads) partitioned by **`(repo, scope, gate)`**, plus content
  storage (blobs/manifests/recipes) in an object store.

The partition key is real: every data-plane record carries its
`(repo, scope, gate)`, and window state is per-partition. What the
partitioning does *not* yet buy is parallelism — all partitions share
one process and one metadata connection, so writes to unrelated
partitions still serialize behind the same lock.

**Deferred** (§7): horizontal scaling across partitions; **deferred**:
`scope_id` is an unvalidated free string today, so partitions are minted
by whatever a client sends (roadmap `g02.014` batch 14.3).

**Deferred** — remote sites get **edge nodes**: read-through caches for
objects and bundle manifests, upload buffering for publishes. Edges hold no
authority — a partitioned edge degrades to read-cached + queued-upload
operation, it never forks policy decisions. This is deliberate: we take
Perforce's lesson (proxies/replicas work well) without federated authority's
unsolved cross-site merge semantics. **No edge code exists**; every mention
of edges in this doc describes the target, not the product.

### Why not federation

Federated multi-site authority requires inventing convergence semantics for
the authority itself (who wins when two sites promote into the same gate
during a partition). Convergence's product already has a convergence
mechanism — gates. Keeping authority central and letting *gates* be the place
where distributed work converges keeps the hard problem in the product's own
vocabulary. Revisit only if a real customer boundary (air-gapped sites,
sovereignty) demands it; record as an explicit new boundary then.

## 2. Storage model

Two traits, both with embedded and external implementations selected by
config — same binary, "lighter deployments" per the vision:

- **`MetadataStore`** — control-plane records + per-partition convergence
  state. Embedded: SQLite (one file per deployment). External: Postgres.
  Every mutation is a scoped transaction touching only its own rows — the
  g01 whole-repo `repo.json` rewrite pattern is structurally impossible.
  Both implementations serialize all writers behind one connection
  mutex; scoping is a property of the *statements*, not yet of the
  concurrency.
- **`ObjectStore`** — content-addressed blobs/manifests/recipes/snap records.
  Embedded: local FS with sharded fanout (`objects/ab/cd/<hash>`). External:
  S3-compatible. Objects are immutable and hash-verified on read (carried
  g01 discipline, now on both sides of the wire).

Metadata references objects by ID; objects never reference metadata. GC is a
mark phase (reachability from lane heads, bundles, releases per retention
policy) followed by an object-store sweep, protected by upload pins and a
grace window (roadmap `g02.012`).

Retention decisions are scoped to the triggering repo, but **the mark
phase walks every repo's roots and the sweep lists the whole object
store** — the object store is shared and deduplicated across repos, so a
narrower mark would sweep another repo's live content. GC also runs
inline on the request thread. Partition-scoped, off-thread GC is
**deferred** to roadmap `g02.014` batch 14.4.

## 3. Consistency model

- **Control plane: linearizable.** Identity, ACL, and gate-graph changes are
  serialized per repo. Volume is low; correctness is the product. (Today
  they are serialized globally, which is strictly stronger.)
- **Partition `(repo, scope, gate)`: serialized writes.** All mutations to
  one partition (publish intake, bundle production, promotion) commit as one
  guarded transactional batch: the writer reads partition state, computes in
  memory, then commits writes together with assertions that the partition and
  its publication window are unchanged (`AssertPartitionState`,
  `AssertPublicationCount`). A tripped assertion rolls the whole batch back;
  publish re-reads and rebuilds under a bounded retry, promote surfaces the
  conflict. This optimistic scheme — not a row-lock single writer — is the
  serialization mechanism in both backends (SQLite `BEGIN IMMEDIATE`,
  Postgres explicit transactions). Publications to the same gate from many
  clients therefore have a total order, which makes bundle input sets
  deterministic.
- **Promotion is monotonic.** Promote only advances the window: it requires
  `bundle.window.last > floor` and that the bundle's base equals the
  partition's current W. A stale bundle — built before the current W was
  promoted — is refused instead of rewinding the floor and re-opening
  consumed publications. Re-promoting the current W to a further downstream
  gate records the promotion without touching partition state (fan-out).
- **Cross-partition: eventually consistent, converged by gates.** A promotion
  from gate A to gate B is an atomic write in B's partition referencing an
  immutable bundle from A's. No transaction spans partitions; immutability of
  inputs makes that safe.
- **Object uploads: idempotent.** Content addressing means duplicate upload
  is a no-op (`write_if_absent` carried to the server side).

## 4. Authorization (enforced this time)

g01 admitted gate/scope ACLs were never enforced. Rebuild rule: **every data
plane operation names its `(repo, scope, gate)` and passes one authz check
before touching state.** Roles are declarative grants stored in the control
plane: subject (user/team) × scope pattern × capability (`read`, `snap-sync`,
`publish`, `resolve`, `approve`, `promote`, `release`, `admin`).
Capability implication is minimal and explicit: a requested `snap-sync`
is satisfied by a `snap-sync`, `publish`, or `admin` grant (publishing
subsumes syncing unpublished work); every other capability is satisfied
only by itself or `admin`. Lane namespace rule: `personal/<subject>` is
reserved — the server refuses client-supplied creation of another
subject's personal lane and auto-provisions the caller's own. No endpoint
ships before its grant check exists — enforced by making the authz context a
required constructor argument of every data-plane handler.

Scope patterns are **literal equality or `*`**; no globbing, and no
registry constrains which scopes exist (roadmap `g02.014` batch 14.3).

**Deferred** — identity: bearer tokens map to subjects through a static
map loaded at startup. They do not expire, carry no capabilities of
their own, and cannot be revoked without a restart. Short-lived
capability-scoped tokens, and edges validating them against the control
plane with offline grace bounded by token TTL, are target state (§7).
Authorization itself — the grant checks — is fully enforced; it is
*authentication* that is slice-grade.

## 5. Bundle coalescing at scale

g01 stubbed the core operation (bundle = input list, no computed manifest).
Semantics live in doc 17; the scale posture:

- A bundle build is a **base-aware 3-way merge** folded onto W (the last
  promoted bundle) over the partition's current **window** of publications
  (doc 17 §2-3). Windows keep input sets small by construction; promotion
  advances the window floor.
- Merge cost is bounded by *changed* paths: deltas are computed against
  each publication's declared base with Merkle short-circuit. Window
  publications are totally ordered (see §3), so bundle builds are
  deterministic: `bundle_id = hash(gate, W root, window ids, strategy,
  merged root)`.
- Divergence resolution is the gate's **coalesce strategy** (doc 17 §4),
  recorded in bundle provenance.
- Bundle builds run **synchronously inside the publish request**: the
  publication and its resulting bundle commit in one guarded batch (§3),
  so publish latency includes the merge. The `BundleStatus::Building`
  state exists in the model but is never constructed — a bundle is
  `Ready` or `Failed` by the time publish returns. **Deferred** (roadmap
  `g02.014` batch 14.2): async partition workers, fast publish
  acknowledgement, and the `building → ready/failed` transition
  observable through the event feed.

## 5b. Event feed (g02.010)

The server records convergence events (`bundle` built, `lane` head moved,
`release` cut) with a per-repo monotonically increasing sequence.

- **Events are hints, never the source of truth**: clients reconcile via
  the inbox/status surfaces; a missed event costs freshness, not
  correctness. At-most-once delivery is therefore acceptable.
- Exposure: `GET /api/repos/:repo/events?since=<seq>` (read capability),
  returning at most one page (1000 events) after the cursor per call
  (g02.011) — the cursor continues, so a gap-recovery poll pages
  through rather than replaying the whole history in one response.
  Poll is the v1 transport; an SSE stream over the same feed is
  deliberate follow-up once external backends land (the feed contract —
  seq cursor + reconcile-on-gap — does not change).
- The events table is **append-only and unbounded**: nothing prunes it,
  so it grows with repo activity forever. Retention with cursor-gap
  signalling is **deferred** to roadmap `g02.014` batch 14.4.

## 5c. Backend selection (operators)

One binary, backends by flag (embedded defaults when omitted):

```bash
# embedded (default): SQLite + local FS under --data-dir
converge-server --addr 0.0.0.0:8080 --data-dir ./data

# external (requires backend-postgres / backend-s3 build features)
converge-server \
  --metadata postgres://user:pass@db:5432/converge \
  --objects  "s3://converge-objects?endpoint=http://minio:9000&region=us-east-1"
```

S3 credentials come from the standard AWS environment variables. The
backend conformance test suite runs against embedded stores always and
against external backends when `CONVERGE_TEST_POSTGRES_URL` /
`CONVERGE_TEST_S3_*` are set.

## 6. Failure and scale posture

Current posture is a single process: it is the unit of availability and
the write ceiling. One crash stops the deployment; one slow merge slows
every concurrent publisher. That is acceptable for the beachhead
(binary-heavy small teams, vision doc 001) and is what the scale
walls roadmap measures rather than assumes.

Target posture, once §7 lands:

- Control plane HA via the metadata backend (Postgres replication /
  single-node embedded accepts its own blast radius).
- Data plane scales by partition count; hot gates can move to dedicated
  workers without model change.
- Edge loss degrades locality, never correctness.

Explicit non-goals at every stage: multi-master authority, offline *policy*
decisions, cross-repo transactions.

## 7. Target architecture (deferred)

Everything here is designed and *not built*. Each item names what would
trigger building it, so the list stays a plan rather than a wish.

| Property | State | Owner / trigger |
| --- | --- | --- |
| Async bundle builds, partition workers | not built; publish merges inline | roadmap `g02.014` batch 14.2 |
| Scope registry, real grant patterns | not built; free-string scopes, literal grants | roadmap `g02.014` batch 14.3 |
| Event retention, off-thread partition-scoped GC | not built; unbounded events, global inline GC | roadmap `g02.014` batch 14.4 |
| Horizontal scaling across partitions | not built; one process, one metadata connection | backlog; trigger = measured write ceiling from the scale-walls roadmap |
| Edge nodes (read-through cache, upload buffering) | not built | backlog; trigger = a real multi-site customer with locality pain |
| Short-lived capability-scoped tokens, revocation | not built; static startup token map | backlog; trigger = any deployment outside a trusted network |

The pluggable-backend seam, the partition key, guarded transactional
writes, and enforced authz are deliberately *not* on this list: they
ship today, and they are what make the deferred items additive rather
than rewrites.

## Open questions carried forward (deferred, with rationale)

- Exact partition-worker mechanism (DB row locks vs dedicated workers):
  decide against real write patterns; both fit the model above.
- ~~Superposition merge policy per entry kind~~ — resolved by doc 17
  (decision table + per-gate strategies).

## Next Task

First rebuild implementation roadmap builds the storage traits and one
vertical slice: publish intake → deterministic bundle build → promote, with
authz enforced end to end.
