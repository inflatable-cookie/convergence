# Convergence Documentation

Northstar-aligned documentation authority for Convergence.

## Core structure

- `vision/`: long-horizon product direction and operating intent
- `architecture/`: durable system model and invariants (canonical object model)
- `contracts/`: explicit working and behavior contracts
- `specs/`: active strict planning and ready-card execution control
- `research/`: comparative systems research findings (dossiers, memos, tracks)
- `rebuild/`: g01-era capture artifacts (lessons, TUI UX spec, salvage)
- `roadmaps/`: segmented executable milestones and backlog
- `logs/`: month-sharded execution history and decision/rationale records
- `git-podcast/`: origin rationale summary
- `policy/`: writing style and docs QA policy inputs

The g01-era docs (operators, processes, testing, extended architecture set,
research scaffolding, g01 roadmap files) are archived on branch `archive/g01`.

## Current state

- Canonical roadmap execution now lives under `roadmaps/g02/`.
- Historical decision records now live under `logs/YYYY-MM/`.
- New roadmap work should use roadmap IDs such as `g02.001`.
- New rationale records and implementation batch notes should go in `logs/YYYY-MM/`.

## Effigy-First Loop

From the repo root:

```bash
effigy tasks
effigy doctor
effigy health
effigy validate
effigy qa:docs
```

Use `effigy test --plan` before test-focused work; the repository test default
prefers `cargo nextest` when it is available on the machine.

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Next Task

Execute the ready `g02.003` Batch 3.1 card
(`roadmaps/g02/batch-cards/006-workspace-scaffold-and-model.md`).002` Batch 2.4 rebuild-architecture card
(`roadmaps/g02/batch-cards/005-rebuild-architecture.md`).
