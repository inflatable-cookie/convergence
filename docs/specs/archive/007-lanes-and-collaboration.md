# 007 Lanes and Collaboration

Status: complete
Updated: 2026-07-24
Roadmap: `g02.007`

## Governing Refs

- `docs/roadmaps/g02/007-lanes-and-collaboration.md`
- `docs/architecture/14-server-authority-and-distribution.md` (grants)
- `docs/architecture/17-lineage-and-merge-semantics.md` (lane heads carry
  lineage)
- `docs/contracts/001-working-rules.md`

## Lane Focus

Lanes stop being a free string: registry, ownership, ACLs, unpublished
sync, inbox. Anything touching merge or lineage semantics routes through
doc 17 before code.

## Current State

- Batch 7.1 (lane model and registry) complete.
- Batch 7.2 (unpublished sync) complete.
- Batch 7.3 (inbox) complete.
- All four batches complete; roadmap `g02.007` closed.

## Exit Condition

Roadmap `g02.007` exit criteria met.

## Next Task

Superseded by `docs/specs/008-releases-retention-and-gc.md`.
