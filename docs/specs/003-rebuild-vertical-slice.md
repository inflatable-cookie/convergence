# 003 Rebuild Vertical Slice

Status: active
Updated: 2026-07-23
Roadmap: `g02.003`

## Context

First implementation lane of the rebuild. Architecture is fixed
(`docs/architecture/13-16`); this spec controls execution grammar only.

## Governing Refs

- `docs/architecture/13-rebuild-workspace-and-crates.md`
- `docs/architecture/14-server-authority-and-distribution.md`
- `docs/architecture/15-client-and-tui-architecture.md`
- `docs/architecture/16-sync-protocol-and-chunking.md`
- `docs/roadmaps/g02/003-rebuild-implementation-vertical-slice.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

Build in the batch order of the roadmap; each batch lands compiling,
validated, and committed before the next card opens. No pre-1.0 compat
shims; the archive branch is history, not a migration source (g01 recipes
and dev-server data do not migrate).

## Batch Model

- one ready card at a time
- implementation only from a ready card
- architecture changes discovered mid-batch go back through
  `docs/architecture/` before code relies on them

## Current State

- Batch 3.1 (workspace scaffold and model) complete.
- Batch 3.2 has a ready card:
  `docs/roadmaps/g02/batch-cards/007-client-core.md`

## Exit Condition

Roadmap `g02.003` exit criteria met: vertical slice end to end with authz,
chunking property tests and sync e2e green.

## Next Task

Execute the ready Batch 3.2 client-core card.
