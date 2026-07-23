# Convergence Documentation

Northstar-aligned documentation authority for Convergence.

## Core structure

- `vision/`: long-horizon product direction and operating intent
- `architecture/`: system model, invariants, and technical boundaries
- `contracts/`: explicit working and behavior contracts
- `specs/`: active strict planning and ready-card execution control
- `research/`: comparative systems research, translation memos, and implementation bridge artifacts
- `roadmaps/`: segmented executable milestones and backlog
- `logs/`: month-sharded execution history and decision/rationale records
- `operators/`: deployment and runtime operations guidance
- `processes/`: contributor and agent working rules
- `git-podcast/`: source analysis and external framing material
- `testing/`: manual and exploratory test guides

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

Execute the ready `g02.002` Batch 2.2 archive-cut card
(`roadmaps/g02/batch-cards/003-archive-cut.md`) on operator go.
