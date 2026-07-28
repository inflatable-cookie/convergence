# 18 Git Interop

Status: active
Updated: 2026-07-24
Roadmap: `g02.009` Batch 9.1

Decision-complete mapping contract between Convergence and git. Nobody
migrates cold: the beachhead teams keep git history and tooling while
Convergence owns binary-heavy and gated workflows. Batches 9.2-9.4
implement this doc.

## 1. Identity correspondence

Git hashes and Convergence ids never coincide (different hash domains).
Correspondence is carried explicitly:

- exported commits carry trailers: `Converge-Snap: <snap_id>` or
  `Converge-Bundle: <bundle_id>`, plus `Converge-Derived-From-Bundle`
  when set
- imported snaps record their source in the message trailer
  `Converge-Imported-Commit: <sha>`
- the exporter maintains a local mapping table
  (`.converge/git-map.json`: snap/bundle id -> exported mark/sha) so
  re-exports are incremental and stable

## 2. Export mapping (Convergence -> git)

- **Snap -> commit.** Tree = the snap's materialized tree, byte-exact.
  Modes map 0o644/0o755; symlinks map; chunked files reassemble into one
  git blob. Message = snap message (or `snap <id[..12]>`), plus trailers.
  Author/committer = the exporting user's git identity; timestamp = snap
  `created_at` (metadata, display-only — identity rides the trailer).
- **Parents.** Snap parents map to commit parents. A thinned ancestor
  (missing record) is omitted and counted in a
  `Converge-Thinned-Parents: <n>` trailer — gaps are visible, never
  faked.
- **Bundle -> merge commit.** Parents = the mapped commits of the
  bundle's input snaps (plus the previous export of the same branch, if
  any). Trailer carries the bundle id; the message includes gate,
  strategy, and window from provenance.
- **Tombstones/deletions.** Absent paths — git-native.
- **Superpositions are unrepresentable in git.** Export **refuses** any
  tree containing a superposition ("resolve before export"). Only
  promotable bundles and superposition-free snaps export. No conflict
  markers, no lossy flattening.

### Branches

- `converge/lane/<lane_id>` — a lane head's lineage
- `converge/release/v<version>` — a release, as a tag-like ref
  (**Deferred**: only lane refs are exported today; release refs follow
  the g02.028 semver identity when somebody needs them in git)

Mirror branches are **read-only for git users**: the exporter force-moves
them on re-export (snapshot semantics). Local branches based on mirrors
are the user's own business; nothing flows back automatically (non-goal:
bidirectional sync).

### Fidelity contract

`git checkout` of an exported commit produces a tree byte-identical to
`converge fetch`/materialize of the corresponding snap or bundle. That
equality is the 9.2 test.

## 3. Import mapping (git -> Convergence)

- **Seed (default).** `converge import` in a git worktree captures the
  current tree as the initial snap, message
  `imported from git <sha[..12]>` + trailer. Depth 1: no history walk.
- **History (`--depth N` / `--all`).** Walk the **first-parent** chain of
  HEAD, oldest first, creating one snap per commit with real lineage
  (parents wired), messages preserved + trailer. Merge side-branches are
  not imported (first-parent keeps lineage linear and cheap; documented
  limitation).
- **Ignore translation.** Import generates `.convergeignore` from the
  repo root `.gitignore` (root-level patterns only; nested .gitignores
  and negations are a documented limitation). Capture honors
  `.convergeignore` alongside its built-ins (this is the one capture
  change 9.3 makes).
- **Author mapping.** Git author is preserved in the imported message
  trailer only; snaps are local records and carry no subject.

## 4. Coexistence rules

- `.git` and `.converge` live in one tree. Capture already ignores
  `.git`; the exporter adds `.converge/` to `.git/info/exclude` so git
  never sees Convergence internals.
- Ownership boundary is by workflow, not path: git remains authoritative
  for whatever the team still drives through git; Convergence owns
  capture, lanes, gates, releases. The mirror branches are the bridge.
- Non-goals (staged out): bidirectional live sync, submodule traversal,
  LFS translation (LFS pointer files export/import as the small pointer
  blobs they are).

## 5. Tooling shape (for 9.2/9.3)

- Export drives `git fast-import` as a child process (stream over stdin,
  deterministic marks); no libgit2 dependency.
- Import shells out to `git` plumbing (`rev-list`, `ls-tree`,
  `cat-file --batch`) — read-only against the git repo.
- Both are client-side (`converge-client` + CLI verbs `git export`,
  `import`); the server knows nothing about git.

## 6. Coexistence quickstart

```bash
cd my-git-repo            # existing git project
converge init
converge git import --all # first-parent history becomes snap lineage
converge login --url ... --repo ... --scope ... --gate ...
converge watch &          # continuous capture from here on
# ... work; publish/promote/release through Convergence ...
converge git export       # mirror lineage to converge/lane/local
git log converge/lane/local   # plain git consumes the mirror
```

Rules that keep the two systems from fighting: capture ignores `.git`;
export excludes `.converge` from git; mirror branches are read-only and
force-moved; the workspace root must be the git worktree root.

## Next Task

Implement export (9.2), import (9.3), coexistence polish (9.4) against
this contract.
