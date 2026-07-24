# 17 Lineage and Merge Semantics

Status: active
Updated: 2026-07-25
Roadmap: `g02.005` Batch 5.1, `g02.015` Batch 15.4, `g02.016` Batch 16.1

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
snap_id = blake3("converge-snap-v3\n"
                 + root_manifest + "\n"
                 + le64(parents.len())
                 + concat(le64(p.len()) + p for p in parents) + "\n"
                 + le64(derived.len()) + derived)
```

Every variable-length field is length-prefixed: a separator-joined parent
list lets different parent splits hash identically once ids are not
fixed-width, which would be a lineage forgery primitive.

Consequences (all intended):

- identity is content + lineage; the timestamp can never fork identity
- capturing a tree identical to the head snap's tree creates nothing:
  `create_snap` returns the head record (a child differing only by lineage
  would be a duplicate in product terms, so it is never created)
- `message` is editable after capture without changing identity — so an
  explicit message given to a recapture of an unchanged tree lands on the
  head record instead of being dropped or forcing a phantom lineage node
- with no head (fresh or detached workspace), recapture dedups against an
  existing parentless snap of the same tree, so repeated auto-captures do
  not accumulate identical records
- records are stored write-once: ids cover tree and lineage only, so a
  second writer's differing metadata must not clobber the stored record;
  message edits use the explicit update path
- history rendering orders by lineage (parent walk), falling back to
  `created_at` only for display of parallel branches

Thinning (automatic capture, g02.006): removing an automatic snap record
leaves its id in surviving children's `parents` as a **thinned ancestor** —
expected, not an error. Identity is unaffected (ids embed parent ids as
opaque strings, not references that must resolve). Re-parenting survivors
is impossible by construction (it would change their ids) and is never
attempted. History rendering walks lineage until a gap and orders anything
unreachable by timestamp; thinning keeps newest-per-bucket, so gaps sit in
old history where the two orders coincide. Explicit snaps and the head are
never thinned.

Head rules: the workspace tracks one head snap id. Capture sets head to the
new snap. Restore sets head to the restored snap. Materializing a bundle
into the workspace sets head to the snap subsequently captured from it
(with `derived_from_bundle` set); materialize alone does not move head.

Implemented (batch 16.1) as two client operations with that split baked
in: `capture_tree` records a stored tree as a snap and leaves head alone,
`adopt_tree` materializes into the workspace first and then moves head.
`resolve apply` uses the second by default (`--no-checkout` selects the
first), so the workspace and head never disagree about what is checked
out.

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

Cost follows from that, in all three phases:

- **Read**: the diff returns as soon as two subtree ids match, so an
  untouched directory is never opened. The values the fold needs from W
  or from another input's base are fetched by walking down the specific
  contested path, not by flattening a tree. Those walks are memoized by
  (root, path) for the fold's lifetime: the supersession rule below asks
  every input's base about every contested path, and a window's inputs
  usually share one base, so without the memo the *window* — not the
  tree — becomes the quadratic term.
- **Write**: the merged tree rewrites only the manifests along changed
  paths; every untouched subtree keeps its existing manifest id, so it
  is neither re-hashed nor re-stored.
- **Classification**: whether the result carries superpositions is
  known from the fold itself — W is superposition-free by construction
  (promote refuses a non-promotable bundle), so no second walk is
  needed to find out.

The measurable consequence, pinned by tests: a one-file publish costs
the same number of manifest reads against a 5-directory tree and a
50-directory one, and a publish whose tree equals its base reads a
single manifest.

At scale (batch 15.4 benchmarks, `effigy bench`): a one-file edit reads
9 manifests whether the tree holds 5k or 50k paths; a 100-publish window
where every publish touches a different directory reads 801 — flat per
publish, and identical on both tree sizes; a 50-publish window that
changes nothing reads 1.

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

- **Supersession by base containment.** A `Set(k)` opinion on path `p` is
  dropped when another input's declared base already contains `k` at `p`
  — either as the value there, or as one of the variants of a
  superposition there (see below) — that publisher demonstrably built on
  top of the value — **and**
  the drop cannot lose content: either that other input expresses its own
  explicit opinion at `p` (a different `Set` or a `Delete`, which is
  causally newer and wins cleanly), or W already carries `k` at `p` (the
  fold preserves it). A silent superseder over a value W does not carry
  leaves the original `Set` in place — it is the only carrier of the
  content. Supersession applies only to `Set` opinions: deletions cannot
  be causally ordered this way (an absent path in a base is
  indistinguishable from never-existed) and always fold per the table.
- **Restating W is still an opinion against a concurrent deletion.** A
  `Set(k)` where `k` equals W's current value collapses into W only when
  no input deletes the path. Against a concurrent `Delete`, that `Set`
  is an explicit keep: the path superposes with the W-valued variant and
  a `Tombstone` — never a silent delete of content someone just
  affirmed. (A deleter whose declared base already contains exactly `k`
  is causally newer and still wins cleanly via supersession above.)
- **A resolution supersedes the variants it decided among.** Base
  containment counts variant membership: a publisher whose declared base
  holds a `Superposition` at `p` saw every variant there and chose. The
  losing variants are superseded even though the base's *value* at `p`
  was the superposition rather than any one of them. Without this rule a
  resolution published into a still-open window (nothing promoted, so
  the original publications re-merge) immediately re-superposes, and
  resolution is impossible before promotion — which is the one moment it
  is most needed. The safety condition is unchanged and does the work:
  the superseder carries its own explicit opinion at `p`, so no content
  is dropped that nothing else expresses. A publisher who never based on
  the superposed bundle is untouched — their opinion was formed without
  seeing the variants, so it still contests the resolution.
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

- three-way line merge (diff3) of the ancestor content vs each variant,
  folded pairwise in input order. The ancestor is the divergent opinions'
  shared declared-base value when they agree (the common content they all
  diverged from — W is irrelevant to intra-window divergence); otherwise
  the fold's current W value; otherwise empty.
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
