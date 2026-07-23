# 14 Server Authority and Distribution

Status: active
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.4

The server design for the rebuild. This doc owns the claim the g01 era never
honored: large distributed development organizations as the primary target.
Operator decisions (2026-07-23): **central control plane with a partitioned
data plane**, shipped as **one binary with pluggable storage backends**.

Research anchors: the Perforce dossier (centralized fragility, single-server
write ceiling, offline pain) and the Jujutsu dossier (conflicts-as-data,
operation log) — `docs/research/specimen-dossiers/`.

## 1. Shape

Two planes, one binary (services enabled by role config):

- **Control plane** — the single logical authority for identity, permissions,
  repo registry, gate graphs, scope/lane registries, and provenance. Small
  data, strong consistency, low write volume. This is what "central server is
  authority" means in the vision — and it is the only part that must be
  globally consistent.
- **Data plane** — convergence state (publications, bundles, promotions,
  releases, lane heads) partitioned by **`(repo, scope, gate)`**, plus content
  storage (blobs/manifests/recipes) in an object store. High volume,
  horizontally scalable, no global locks.

Remote sites get **edge nodes**: read-through caches for objects and bundle
manifests, upload buffering for publishes. Edges hold no authority — a
partitioned edge degrades to read-cached + queued-upload operation, it never
forks policy decisions. This is deliberate: we take Perforce's lesson
(proxies/replicas work well) without federated authority's unsolved cross-site
merge semantics.

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
  Every mutation is a scoped transaction on its partition — the g01
  whole-repo `repo.json` rewrite pattern is structurally impossible.
- **`ObjectStore`** — content-addressed blobs/manifests/recipes/snap records.
  Embedded: local FS with sharded fanout (`objects/ab/cd/<hash>`). External:
  S3-compatible. Objects are immutable and hash-verified on read (carried
  g01 discipline, now on both sides of the wire).

Metadata references objects by ID; objects never reference metadata. GC is a
partition-scoped mark phase (reachability from lane heads, bundles, releases
per retention policy) followed by object-store sweep with a grace window —
never a global stop-the-world pass.

## 3. Consistency model

- **Control plane: linearizable.** Identity, ACL, and gate-graph changes are
  serialized per repo. Volume is low; correctness is the product.
- **Partition `(repo, scope, gate)`: serialized writes.** All mutations to
  one partition (publish intake, bundle production, promotion) go through a
  single writer — a DB transaction with a partition row lock, or a
  per-partition worker. Publications to the same gate from many clients
  therefore have a total order, which makes bundle input sets deterministic.
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
`publish`, `resolve`, `approve`, `promote`, `release`, `admin`). Edges
enforce read grants on cached content by validating tokens against the
control plane (short-lived, capability-scoped tokens; offline edge grace
bounded by token TTL). No endpoint ships before its grant check exists —
enforced by making the authz context a required constructor argument of every
data-plane handler.

## 5. Bundle coalescing at scale

g01 stubbed the core operation (bundle = input list, no computed manifest).
Rebuild design:

- A gate's bundle build is a **manifest merge** over its ordered input set:
  walk input root manifests, path-by-path; identical entries pass through;
  divergent entries become `Superposition` nodes with per-variant provenance
  (model already supports this).
- Merge cost is bounded by *changed* paths: manifests are Merkle trees, so
  identical subtree hashes short-circuit whole directories. Input sets are
  totally ordered (see §3), so bundle builds are deterministic and
  reproducible from provenance.
- Bundle builds run in the partition's worker, async from publish intake;
  a publication is accepted fast, coalescing follows. Status is visible
  (`building` → `ready`/`failed`) — no silent stubs.

## 6. Failure and scale posture

- Control plane HA via the metadata backend (Postgres replication /
  single-node embedded accepts its own blast radius).
- Data plane scales by partition count; hot gates can move to dedicated
  workers without model change.
- Edge loss degrades locality, never correctness.
- Explicit non-goals at this stage: multi-master authority, offline *policy*
  decisions, cross-repo transactions.

## Open questions carried forward (deferred, with rationale)

- Exact partition-worker mechanism (DB row locks vs dedicated workers):
  decide in the first server implementation roadmap against real write
  patterns; both fit the model above.
- Superposition merge policy per entry kind (file vs dir vs symlink edge
  cases): specify alongside the coalescing implementation, driven by tests.

## Next Task

First rebuild implementation roadmap builds the storage traits and one
vertical slice: publish intake → deterministic bundle build → promote, with
authz enforced end to end.
