# 001 Lessons Retrospective (g01 Era)

Status: capture artifact
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.1

Retrospective over the g01-era implementation (2026-01 → 2026-05, 373 commits,
~30k LOC) ahead of the archive cut. Evidence: full docs + code surveys,
2026-07-23.

## What Worked

### Product theory (validated, keep)

- Cheap, low-expectation snaps decouple state capture from code quality. The
  quality gate moves to publish time, with convergence handled by a managed
  pipeline. This is the core UX bet and it held up.
- The six-verb contract (`snap`, `publish`, `bundle`, `promote`, `release`,
  `superposition`) stayed stable across all docs and code. Terminology
  discipline paid off.
- Superposition-as-data — unresolved conflict as a first-class manifest node
  with per-variant provenance — is the genuinely novel idea, and it survived
  contact with implementation (`ManifestEntryKind::Superposition`).

### Engineering discipline (keep)

- Content-addressed blake3 Merkle DAG with verify-on-read everywhere. Integrity
  was real, not aspirational.
- Atomic temp+rename writes and `write_if_absent` dedup in the local store.
- Client/server seam stayed honest: server consumed only `converge::model`;
  contract was HTTP+JSON. Made the rebuild split cheap.
- The comparative research program (Git, Mercurial, Perforce, Plastic SCM,
  Jujutsu dossiers + translation memos) produced decision-grade material that
  directly shaped the object model.

### TUI UX (keep as spec)

- The interactive shell UX was good: the flows and views worked. Captured
  implementation-independent in [`002-tui-ux-spec.md`](./002-tui-ux-spec.md).

## What Failed

### Server architecture

- The vision claimed large-organization workflows as the primary target; the
  implementation was a single-node dev fixture: in-memory `RwLock<HashMap>`
  state, whole-repo `repo.json` rewritten on every mutation, flat blob dirs,
  no DB, no sharding, no replication, gate/scope ACLs never enforced.
- The architecture docs never reconciled that gap — server authority got a
  42-line conceptual doc while the Perforce research explicitly warned about
  centralized single-server fragility. Lesson: the hardest claim needs the
  deepest design doc, not the thinnest.

### TUI implementation

- 16.5k LOC / 332 files — over half the codebase — for the UI layer, in
  hyper-fragmented ~50-line files. UI iteration churn also consumed ~20 of 46
  roadmap files. Lesson: cap UI implementation investment until the engine
  underneath is settled; capture UX as spec early.

### Docs process weight

- 134 doc files, of which only ~10-15 carry durable product substance. The
  bulk was process governance, pause apparatus, and README wrappers. Research
  scaffolding (templates, hubs) outweighed research findings. Lesson: spine
  stays lean; governance surfaces should not outnumber product surfaces.

### Technical shortcuts that would not survive scale

- Fixed 4 MB block chunking (no rolling hash) — weak dedup on edits.
- Flat unsharded object directories — degrades at large object counts.
- Bundles never computed coalesced manifests from inputs (admitted MVP gap) —
  the convergence pipeline's core operation was stubbed.

## Open Questions (feed Batch 2.4)

- Distributed authority model: what is the honest consistency/replication
  design for gates and scopes across sites? Federation vs primary-with-edge
  caching vs partitioned authority per `(repo, scope, gate)`?
- Storage backend: which object store + metadata DB shape fits the
  publish/bundle/promote write patterns?
- How does bundle coalescing actually compute merged manifests with
  superpositions at scale?
- Identity/authz: real gate/scope ACL enforcement design.
- Chunking: content-defined chunking choice (FastCDC or similar) and migration
  of the recipe format.

## Next Task

See [`003-salvage-inventory.md`](./003-salvage-inventory.md) and the governing
spec `docs/specs/002-archive-and-rebuild-boundary.md`.
