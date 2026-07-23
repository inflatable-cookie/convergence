# 005 Rebuild Architecture

Status: complete
Updated: 2026-07-23
Roadmap: `g02.002`
Spec: `docs/specs/002-archive-and-rebuild-boundary.md`

## Objective

Define the rebuild architecture: workspace layout, a server design that
honestly serves large distributed systems, and a client design that carries
the validated local UX theory. This is design work, not implementation.

## Inputs

- `docs/rebuild/001-lessons-retrospective.md` (esp. open questions)
- `docs/rebuild/002-tui-ux-spec.md`
- `docs/rebuild/003-salvage-inventory.md`
- `docs/architecture/01`, `04`
- `docs/research/` findings (Perforce centralized-fragility warnings apply
  directly to server design)

## In Scope

- Cargo workspace layout: shared `model` crate, client crate, server crate
- server architecture: storage backend (object store + metadata DB),
  replication/federation posture, consistency model for gates/scopes,
  enforced gate/scope authorization, bundle coalescing at scale
- client architecture: CLI core as the canonical verb surface; TUI as thin
  front-end over the CLI argv contract per the UX spec
- sync protocol contract (carried shape: missing-objects negotiation →
  upload → publish)
- content-defined chunking decision and recipe format implications
- promote resulting decisions into `docs/architecture/` as new numbered docs

## Out Of Scope

- writing implementation code
- migrating the salvaged modules into the workspace layout (first
  implementation roadmap owns that)

## Acceptance Criteria

- new architecture docs cover server authority/storage/distribution at least
  as deeply as the client side — the g01 failure mode (thinnest doc for the
  hardest claim) does not repeat
- every lessons-retrospective open question is either answered or explicitly
  deferred with rationale
- exit criteria for `g02.002` are satisfied so the first rebuild
  implementation roadmap (`g02.003`) can open

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Stop Conditions

- a server design decision needs operator intent (e.g. deployment topology
  tradeoffs) — ask instead of guessing

## Outcome

- operator decisions taken: central control plane + partitioned
  `(repo, scope, gate)` data plane; one binary with pluggable storage
  (embedded SQLite/FS, external Postgres/S3)
- promoted `docs/architecture/13-16`: workspace/crates, server authority and
  distribution (deepest doc of the set — the g01 failure mode does not
  repeat), client/TUI, sync protocol + FastCDC chunking
- every lessons-retrospective open question answered (authority model,
  storage backend, bundle coalescing, ACL enforcement, chunking) or
  explicitly deferred with rationale in doc 14 (partition-worker mechanism,
  per-kind merge policy)
- `g02.002` exit criteria satisfied

## Next Task

Close `g02.002` and compile the first rebuild implementation roadmap
(`g02.003`).
