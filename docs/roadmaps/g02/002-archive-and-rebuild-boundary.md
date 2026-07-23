# 002 Archive-and-Rebuild Boundary

Status: active
Owner: repo maintainers
Updated: 2026-07-23

## Context

`g02.001` held the repo paused until a real post-research boundary appeared.
That boundary is now explicit: archive the current implementation, keep the
validated concept and lessons, restructure the docs spine, and rebuild client
and server as independent systems in one Cargo workspace.

Assessment evidence and salvage verdicts live in
`docs/specs/002-archive-and-rebuild-boundary.md`.

## Goals

- capture lessons, TUI UX, and salvage inventory before any cut
- archive the g01-era implementation without losing history
- reduce the docs tree to a lean strict spine (~10-15 carried files)
- define the rebuild architecture: shared model crate, client, server, and a
  server design that honestly targets large distributed deployment

## Non-Goals

- rebuilding client or server inside this roadmap (that work gets its own
  roadmap files once architecture is defined)
- carrying TUI implementation code forward
- preserving the dev-server storage layer in any form

## Execution Plan

### Batch 2.1 - Capture

- [x] write lessons retrospective (what worked, what failed, open questions)
- [x] capture TUI UX as a spec: views, flows, interaction model, what made it
      good — implementation-independent
- [x] write salvage inventory pinning exact paths/modules that carry forward

Artifacts: `docs/rebuild/001-lessons-retrospective.md`,
`docs/rebuild/002-tui-ux-spec.md`, `docs/rebuild/003-salvage-inventory.md`

### Batch 2.2 - Archive Cut

- [ ] tag current state `v0-legacy`
- [ ] create `archive/g01` branch
- [ ] strip `main` to docs spine + salvaged code

### Batch 2.3 - Docs Spine Restructure

- [ ] carry the keeper set; dedupe the object model (currently restated 4x)
- [ ] archive g01 roadmap files, pause apparatus, research scaffolding
- [ ] realign front doors, contracts, Effigy QA surfaces

### Batch 2.4 - Rebuild Architecture Definition

- [ ] workspace layout: model / client / server crates
- [ ] server architecture for large distributed systems: storage, metadata DB,
      replication/federation posture, enforced gate/scope authorization
- [ ] client architecture: CLI core + TUI rebuilt against the captured UX spec
- [ ] sync protocol contract carried from salvage verdicts

## Exit Criteria

- archive tag and branch exist; `main` is spine + salvage only
- docs tree is the lean strict spine with coherent front doors
- rebuild architecture is promoted and the first rebuild roadmap can open

## Next Task

Execute the ready Batch 2.2 archive-cut card
(`batch-cards/003-archive-cut.md`) on operator go.
