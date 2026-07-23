# 003 Rebuild Implementation: Vertical Slice

Status: active
Owner: repo maintainers
Updated: 2026-07-23

## Context

`g02.002` closed with the rebuild architecture promoted
(`docs/architecture/13-16`). This roadmap builds the first implementation
increment: the Cargo workspace, the shared model, and one honest vertical
slice — init → snap → publish → deterministic bundle → promote — with authz
enforced end to end.

Governing surfaces: `docs/specs/003-rebuild-vertical-slice.md`,
`docs/architecture/13-16`, `docs/rebuild/003-salvage-inventory.md`.

## Goals

- stand up the five-crate workspace (arch 13)
- migrate salvage into `converge-model` and `converge-client`
- replace fixed-block chunking with FastCDC (arch 16)
- build the server slice on pluggable embedded storage with real authz
  (arch 14)
- prove the wire contract with an end-to-end publish/fetch path

## Non-Goals

- TUI (starts after the CLI surface stabilizes, arch 15)
- external backends (Postgres/S3) — traits land now, implementations later
- edge nodes, workflow profiles, releases/retention polish

## Execution Plan

### Batch 3.1 - Workspace Scaffold and Model

- [x] create the five-crate workspace; move salvaged `model/` into
      `converge-model`; collapse wire DTOs into it
- [x] implement FastCDC chunker + recipe format with property tests
      (insert/delete edit stability)
- [x] `effigy.toml` tasks target the workspace

### Batch 3.2 - Client Core

- [ ] migrate store/diff/resolve/workspace salvage into `converge-client`;
      add sharded object fanout; lift the salvage `allow(dead_code)`
- [ ] snap capture on FastCDC recipes

### Batch 3.3 - CLI Verb Surface

- [ ] `converge-cli`: init, snap, history, diff, resolve verbs with stable
      argv + `--json`

### Batch 3.4 - Server Slice

- [ ] `MetadataStore`/`ObjectStore` traits + embedded impls (SQLite, FS)
- [ ] control plane minimum: identity, capability grants, one repo + gate
      graph; authz context required by every handler
- [ ] publish intake → deterministic bundle coalescing (Merkle merge with
      superposition nodes) → promote, serialized per partition

### Batch 3.5 - End-to-End Sync

- [ ] negotiate/upload/commit protocol client+server; publish and fetch
      round-trip e2e test with dedup and resume assertions

## Exit Criteria

- workspace builds and validates under Effigy
- vertical slice runs end to end against the embedded server with authz on
- chunking property tests and sync e2e tests green

## Next Task

Execute the ready Batch 3.2 card (`batch-cards/007-client-core.md`).
