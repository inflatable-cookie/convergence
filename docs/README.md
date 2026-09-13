# Convergence Documentation

Northstar-aligned documentation authority for Convergence.

## Core structure

- `vision/`: long-horizon product direction and operating intent
- `architecture/`: durable system model and invariants (canonical object model)
- `contracts/`: explicit working and behavior contracts
- `specs/`: active strict planning and ready-task execution control
- `research/`: comparative systems research findings (dossiers, memos, tracks)
- `rebuild/`: g01-era capture artifacts (lessons, TUI UX spec, salvage)
- `roadmaps/`: executable Northstar tasks (`gNN.NNN`); deferred candidates wait in `triage/`
- `logs/`: month-sharded execution history and decision/rationale records
- `guides/`: task-shaped walkthroughs proven by tests
- `git-podcast/`: origin rationale summary
- `policy/`: writing style and docs QA policy inputs

The g01-era docs (operators, processes, testing, extended architecture set,
research scaffolding, g01 roadmap files) are archived on branch `archive/g01`.

## Current state

- Canonical task execution now lives under `roadmaps/g02/`, one file per
  Northstar task `g02.NNN`.
- Historical decision records now live under `logs/YYYY-MM/`.
- New tasks use task IDs such as `g02.032`.
- New rationale records should go in `logs/YYYY-MM/`.

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

The evidence-only `g02.031` installed Rust package canary merged (PR #4).
Its result does not authorize product repair. Product execution remains
paused: `g02.027` awaits the operator's TUI cold-drive verdict, while the
`g02.022` release step has a built pipeline but **no release cut**.
Canonical queue: `roadmaps/g02/README.md`.
<!-- northstar:lifecycle:begin schema=northstar.lifecycle.projection.v2 digest=sha256:c802836a0adce05a6e9df621cbf58427b8756343af64e7dd7078cce508f77235 -->
| Generation | Disposition | Runway state |
| --- | --- | --- |
| g02 | open | planning_required |
| Task | Status | Stage | Revision | Record digest |
| --- | --- | --- | --- | --- |
| g02.032 | complete | none | 8 | sha256:e31ca216ea4ceafcd6576364f17dfcece94a1e16aa5c3ffa4b486495058965a1 |
<!-- northstar:lifecycle:end -->
