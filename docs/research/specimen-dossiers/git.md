# System Dossier: Git

**Product Identity**: Distributed version control system created by Linus Torvalds in 2005 for Linux kernel development.

**Era Context**: Post-BitKeeper, needed something fast for a large distributed project. Designed in weeks, optimized for patch-based mailing list workflow.

---

## Defining Architectural Bets

1. **Immutable object store** — Content-addressed DAG of commits, trees, blobs, tags. Everything is an object.
2. **Distributed by default** — Every clone is a full copy with complete history.
3. **Explicit staging area** — The index allows fine-grained control over what goes into a commit.
4. **Branches are cheap references** — A branch is just a file containing a commit hash.
5. **Merge is the default integration** — Rebase exists but merge is the "safe" path.
6. **Cryptographic integrity** — SHA-1 (now SHA-256) for object identity and trust.

---

## Standout Strengths

- **Speed of basic operations** — Local commits, diffs, and history are fast.
- **Offline workflow** — Full capability without network.
- **Branching model** — Cheap branches enable feature workflows.
- **Ecosystem dominance** — GitHub, GitLab, Bitbucket, every CI/CD tool, every IDE.
- **Data integrity** — Content-addressing prevents silent corruption.
- **Patch-based workflow** — Email-based collaboration still works for kernel.

---

## Chronic Pain Points

1. **Large repository scaling**
   - Full clone required historically (mitigated by partial clones since 2.29)
   - History grows forever
   - `git status` scans entire working directory

2. **Large binary handling**
   - Blobs go in object store forever
   - Git LFS is a bolt-on requiring separate infrastructure
   - No native locking for unmergeable files

3. **Complexity surface**
   - Index/staging is powerful but confusing
   - `git reset` has multiple modes with different semantics
   - Rebase vs. merge is a decision users must make
   - Submodules are notoriously difficult

4. **Monorepo pain**
   - Facebook/Meta used custom patches (VFS for Git) for years
   - Microsoft built Scalar and VFS for Git
   - Google uses Piper (internal) not Git for largest repos

5. **Partial/unsaved work**
   - Stash is a stack of patches, not first-class
   - No native "continuous capture" — must explicitly commit
   - WIP commits are common but messy

6. **Conflict handling**
   - Conflicts are local state, not preserved
   - Rerere helps but is opt-in and limited
   - No way to collaborate on conflict resolution

---

## Between-Release Corrections

| Version | Change | Rationale |
|---------|--------|-----------|
| 2.29+ | SHA-256 migration path | SHA-1 collision concerns |
| 2.29+ | Partial clones (`--filter`) | Large repo scaling |
| 2.25+ | Sparse index | Faster `git status` for sparse checkouts |
| 2.19+ | Commit graph | Faster history traversals |
| 2.x | Worktrees | Multiple working directories from one repo |
| 2.x | Reftable (experimental) | Better ref storage for massive ref counts |

---

## Convergence-Relevant Lessons

### What to study

1. **Object store design** — Git's content-addressed immutable store is elegant and worth understanding.
2. **Reference model** — Branches as mutable refs to immutable commits separates movement from history.
3. **Packfile efficiency** — Delta compression and packfile structure for storage efficiency.
4. **Transport protocols** — Smart HTTP, SSH, git protocol for sync semantics.

### What to reject early

1. **Everything-is-local** — Convergence explicitly wants server authority for gates and identity.
2. **Explicit commit as only capture** — Convergence wants continuous/implicit capture via `snap`.
3. **Merge-centric conflict model** — Convergence wants to preserve conflicts as `superposition` data.
4. **SHA-1/SHA-256 as primary identity** — May want ULIDs or other time-sortable IDs.

### What to prototype/compare

1. **Partial clone vs. lazy loading** — How much workspace state must be local?
2. **Staging area alternatives** — Explicit (Git), automatic (Jujutsu), or continuous (Convergence snap)?
3. **Submodule vs. monorepo** — Convergence's scope/lane model should learn from both.

---

## Source Inventory

| Source | Type | Confidence | Notes |
|--------|------|------------|-------|
| git-scm.com/book | Official docs | High | Pro Git book, free online |
| Git source code | Source tree | High | C, shell, Perl |
| Git Merge talks | Conference | Medium | Some vendor content |
| "Git from the Bottom Up" | Article | High | John Wiegley |
| Facebook/Meta engineering blogs | Postmortem | Medium | VFS for Git scaling |
| Microsoft DevOps blogs | Postmortem | Medium | Scalar, GVFS |

---

---

## Technical Deep Dives

### Packfile Format and Delta Compression

Git stores objects efficiently using **packfiles** — compressed archives with delta compression:

- **Loose objects** — Individual files named by hash (for recent objects)
- **Packfiles** — Compressed archives with delta chains for older objects
- **Delta encoding** — Similar objects stored as deltas against a base
- **Pack index** — `.idx` files for O(log n) lookup in packs

Packfiles use a **sliding window** delta algorithm: objects are sorted by type+size+name, then similar objects are delta-compressed against prior objects in the window. This achieves significant compression for similar files (source code, similar assets).

**Convergence consideration**: Packfiles are read-optimized. Convergence may want different tradeoffs if server-authority allows different storage strategies.

### Transport Protocols

Git supports multiple transport protocols with different characteristics:

| Protocol | Direction | Smart/Dumb | Notes |
|----------|-----------|------------|-------|
| `git://` | Unidirectional | Dumb | Fast, no auth, read-only |
| SSH (`git@`) | Bidirectional | Smart | Auth via SSH keys, full features |
| HTTPS | Bidirectional | Smart | Auth via TLS, most common |
| Local (`file://`) | N/A | N/A | Same filesystem |

**Smart HTTP** protocol (introduced in Git 1.6.6) uses two endpoints:
- `GET /info/refs?service=git-upload-pack` — Discovery
- `POST /git-upload-pack` — Fetch negotiation and pack transfer
- `POST /git-receive-pack` — Push

The protocol negotiates what objects each side has, then generates a custom packfile containing only the missing objects.

**Convergence consideration**: The "smart" negotiation protocol is efficient but complex. Convergence with server authority may prefer simpler sync models.

### Submodule and Subtree Design

Git has two mechanisms for repository composition:

**Submodules** — Separate repos nested within parent:
- Stored as commit hashes in parent repo
- Must be initialized and updated separately
- Requires `.gitmodules` file
- Pain points: recursive operations, detached HEAD states, update friction

**Subtrees** — Merge external repo history into subdirectory:
- Uses normal merge machinery
- Can squash or preserve history
- `git subtree split` extracts subdirectory to its own branch
- Less common but avoids many submodule issues

**Convergence consideration**: Both are workarounds for Git's lack of native monorepo support. Convergence's scope/lane model should learn from these pain points.

---

## Related Convergence Tracks

- Track 1: Continuous capture vs. explicit commit
- Track 2: Gate-based workflows
- Track 4: Bundle semantics
- Track 5: Large repository handling
- Track 6: Large binary workflows
- Track 7: Workspace state management
