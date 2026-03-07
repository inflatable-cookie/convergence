# Convergence Documentation

Northstar-aligned documentation authority for Convergence.

## Core structure

- `vision/`: long-horizon product direction and operating intent
- `architecture/`: system model, invariants, and technical boundaries
- `research/`: comparative systems research, translation memos, and implementation bridge artifacts
- `roadmaps/`: segmented executable milestones and backlog
- `logs/`: month-sharded execution history and decision/rationale records
- `operators/`: deployment and runtime operations guidance
- `processes/`: contributor and agent working rules
- `git-podcast/`: source analysis and external framing material
- `testing/`: manual and exploratory test guides

## Current state

- Canonical roadmap execution lives under `roadmaps/g01/`.
- Historical decision records now live under `logs/YYYY-MM/`.
- New roadmap work should use roadmap IDs such as `g01.043`.
- New rationale records and implementation batch notes should go in `logs/YYYY-MM/`.

## Effigy-First Loop

From the repo root:

```bash
effigy tasks --repo .
effigy health --repo .
effigy validate --repo .
```

Use `effigy test --repo .` for the repository test default, which prefers `cargo nextest` when it is available on the machine.
