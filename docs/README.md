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
- `guides/`: task-shaped walkthroughs proven by tests
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

Use `effigy test --plan` before test-focused work; the configured `rust` suite
uses `cargo nextest run -P ci`.

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Next Task

Run the evidence-only `g02.031/102` installed Rust package canary. Its result
does not authorize product repair. Product execution remains paused:
`g02.027` awaits the operator's TUI cold-drive verdict, while `g02.022` batch
22.5 has a built release pipeline but **no release cut**. Canonical queue:
`roadmaps/g02/README.md`.
