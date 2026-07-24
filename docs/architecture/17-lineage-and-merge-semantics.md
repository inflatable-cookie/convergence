# 17 Lineage and Merge Semantics

Status: active
Updated: 2026-07-24
Roadmap: `g02.005` Batch 5.1

Decision-complete semantics for snap lineage, base-aware merge, bundle
windows, and per-gate coalesce strategies. Supersedes the union-merge and
flat-history semantics of the first vertical slice. Docs 14 and 16 defer to
this doc where they overlap.

Pre-1.0 rule applies: no compatibility shims; existing slice stores re-init.

## 1. Snap lineage

Snaps form a DAG per workspace.

Record (v2):

- `parents: Vec<SnapId>` — ordered, deduplicated. Empty for the first snap
  in a workspace. First parent is the primary lineage: the workspace head
  at capture time. Multiple parents occur only when a capture incorporates
  external history (currently: none produced automatically; reserved).
- `derived_from_bundle: Option<BundleId>` — set when the captured tree is
  the materialization/resolution of a fetched bundle. Bundles are not
  snaps, so this is a provenance edge, not a parent.
- `root_manifest`, `stats` — as today.
- `created_at`, `message`, `trigger` — metadata only.

Identity:

```
snap_id = blake3("converge-snap-v2\n"
                 + root_manifest + "\n"
                 + parents.join(",") + "\n"
                 + derived_from_bundle.unwrap_or(""))
```

Consequences (all intended):

- identity is content + lineage; the timestamp can never fork identity
- capturing an unchanged tree over the same head reproduces the same id —
  `create_snap` returns the existing record instead of writing a duplicate
- `message` is editable after capture without changing identity
- history rendering orders by lineage (parent walk), falling back to
  `created_at` only for display of parallel branches

Head rules: the workspace tracks one head snap id. Capture sets head to the
new snap. Restore sets head to the restored snap. Materializing a bundle
into the workspace sets head to the snap subsequently captured from it
(with `derived_from_bundle` set); materialize alone does not move head.

## 2. Base-aware merge

### Publication base

`PublishRequest` and `PublicationRecord` gain `base_bundle_id:
Option<BundleId>` — the bundle the publisher last knew for the target
`(repo, scope, gate)`. The client records the latest bundle id it has seen
per target (on publish response and on fetch) and sends it automatically.
`None` means "no known base": the input's delta is computed against the
empty tree (everything reads as added). The server rejects a
`base_bundle_id` that is not in the partition's history.

### Window base W

Every bundle build starts from **W**, the root manifest of the partition's
last *promoted* bundle (empty tree if none — see §3 for windows).

### Delta and decision table

For each input publication `i`, the server computes
`delta_i = diff(base_i, tree_i)` per path — added / modified / deleted /
unchanged — manifest-recursively with Merkle short-circuit (identical
subtree hashes prune).

Per path, fold deltas onto W:

| Situation | Result |
| --- | --- |
| no input touches the path | W's entry passes through |
| one input adds/modifies; rest untouched | that value |
| several inputs set the same content | that value (dedup) |
| several inputs set divergent content | strategy dispatch (§4); unresolved → `Superposition`, one variant per distinct content, source = lane |
| one input deletes; rest untouched | path removed from the bundle manifest |
| one input deletes; another modifies | `Superposition` containing the modified variant(s) and a `Tombstone` variant |
| input's delta is `unchanged` for the path | that input expresses no opinion — it never creates a variant |

Rules:

- **Tombstones never appear as plain manifest entries.** A resolved
  deletion is an absent path. `Tombstone` exists only as a superposition
  variant, and resolving a superposition to its tombstone variant removes
  the path.
- Materialization skips nothing: bundle manifests contain only real
  entries and superpositions.
- The "unchanged expresses no opinion" rule is what kills the slice's
  false superpositions: a publisher who didn't touch a file can no longer
  collide with one who did.

## 3. Bundle windows

Partition state gains `window_floor: u64` — the highest publication `seq`
consumed by the last **promoted** bundle (0 initially).

- A bundle build consumes the ordered publications with
  `seq > window_floor` (the *window*).
- Promotion of bundle B sets `window_floor` to the highest seq in B's
  window and makes B the new W for subsequent builds.
- Builds between promotions repeatedly re-merge the current window — small
  by construction.
- Provenance on the bundle records: `base_bundle_id` (W's bundle, if any)
  and the window's `(first_seq, last_seq)`.

Determinism contract:

```
bundle_id = blake3(gate_id, W_root, ordered window publication ids,
                   strategy name, merged_root)
```

Same W, same window, same strategy → same bundle, byte for byte.

## 4. Per-gate coalesce strategies

`GateNode` gains `strategy` (serde default `whole-file`). The strategy is
recorded in bundle provenance. Dispatch is per divergent path after the §2
fold; directories always recurse; only leaf divergence reaches a strategy.

### `whole-file`

Divergent content becomes a superposition (slice behavior). Correct default
for binaries and unknown formats.

### `text-line-merge`

For divergent `File`/`FileChunks` entries where base and all variants are
text (heuristic: no NUL byte in the first 8 KiB, valid UTF-8):

- three-way line merge (diff3) of base content vs each variant, folded
  pairwise in input order
- clean merge → a new blob; the merged entry is a normal `File` (mode: from
  the variants if they agree, else the base's; size recomputed)
- any overlapping-hunk conflict → fall back to a superposition of the
  *original* variants. **No conflict markers are ever written into
  content** — conflicts stay data (product guardrail).
- non-text content under this strategy falls back to `whole-file` per path

Strategies are a closed enum for now (`whole-file`, `text-line-merge`);
custom/domain strategies are a later roadmap and must keep the determinism
contract.

## 5. Wire and model deltas (summary for doc 16)

- `SnapRecord` v2: `parents`, `derived_from_bundle`, identity rule above
- `PublishRequest` / `PublicationRecord`: `+ base_bundle_id`
- `BundleRecord`: `+ base_bundle_id`, `+ window: (u64, u64)`,
  `+ strategy: String`
- `GateNode`: `+ strategy`
- client state: last-seen bundle id per `(repo, scope, gate)` target

## Next Task

Implement in roadmap order: Batch 5.2 lineage, 5.3 base-aware merge and
windows, 5.4 strategies.
