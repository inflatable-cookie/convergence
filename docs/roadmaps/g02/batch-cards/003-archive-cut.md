# 003 Archive Cut

Status: ready (execute on explicit operator go — mutates git surfaces)
Updated: 2026-07-23
Roadmap: `g02.002`
Spec: `docs/specs/002-archive-and-rebuild-boundary.md`

## Objective

Archive the g01-era implementation in git and strip `main` to the docs spine
plus salvage, per the salvage inventory.

## In Scope

- commit current planning + capture state on `main`
- tag that state `v0-legacy`
- create `archive/g01` branch at the same point
- on `main`: remove discard-listed code
  (`src/bin/converge_server/`, `src/tui_shell/`, `src/tui.rs`, `src/remote/`,
  `src/cli_*`, `scripts/`, `dev/`, fixed-block chunking modules) per
  `docs/rebuild/003-salvage-inventory.md`
- keep carry-listed modules compiling as a reduced crate, or park them under a
  clearly-marked salvage layout if a clean build is not yet sensible
- update README/AGENTS/effigy.toml so front doors and tasks match what remains

## Out Of Scope

- docs spine restructure (Batch 2.3)
- workspace crate layout and rebuild architecture (Batch 2.4)
- deleting anything from git history

## Acceptance Criteria

- `v0-legacy` tag and `archive/g01` branch exist and point at the full state
- `main` contains no discard-listed code
- front doors and Effigy tasks reference only surfaces that still exist
- validation passes on the reduced tree

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`
- `effigy health` (if the reduced crate builds; otherwise record the parked
  state explicitly in the closeout log)

## Stop Conditions

- carry modules turn out entangled with discard modules beyond quick
  detachment — update the salvage inventory and spec before cutting

## Next Task

On completion, open the Batch 2.3 docs-spine-restructure card.
