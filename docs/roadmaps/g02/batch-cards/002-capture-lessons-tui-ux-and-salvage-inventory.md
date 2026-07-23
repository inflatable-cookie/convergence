# 002 Capture Lessons, TUI UX, and Salvage Inventory

Status: complete
Updated: 2026-07-23
Roadmap: `g02.002`
Spec: `docs/specs/002-archive-and-rebuild-boundary.md`

## Objective

Capture everything worth keeping from the g01-era implementation before the
archive cut removes it from `main`.

## In Scope

- lessons retrospective: what worked (model, store discipline, research
  program), what failed (server storage layer, TUI sprawl, docs
  process-weight), open questions (distributed authority model)
- TUI UX spec: views, flows, interaction model, and what made the UX good —
  written implementation-independent so the rebuilt TUI can target it
- salvage inventory: exact modules that carry forward (`src/model/`,
  `src/store/`, `src/diff/`, `src/resolve/`, sync protocol shape) with any
  known caveats (flat object dir sharding, fixed-block chunking replacement)

## Out Of Scope

- deleting or moving any code (Batch 2.2)
- docs spine restructure (Batch 2.3)
- rebuild architecture design (Batch 2.4)

## Acceptance Criteria

- three capture documents exist under the docs tree and are linked from the
  spec
- the TUI UX spec is reviewable without reading `src/tui_shell/`
- the salvage inventory names concrete paths, not vibes

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Stop Conditions

- capture reveals a salvage verdict is wrong (e.g. a "carry" module is
  entangled) — update the spec before proceeding

## Outcome

- `docs/rebuild/001-lessons-retrospective.md` — validated theory, failed
  server/TUI-sprawl/docs-weight lessons, open questions for Batch 2.4
- `docs/rebuild/002-tui-ux-spec.md` — shell model, views, keys, flows, eight
  preserved interaction principles, seven warts flagged for rebuild
- `docs/rebuild/003-salvage-inventory.md` — concrete carry/discard paths
- All three linked from the governing spec

## Next Task

Execute the Batch 2.2 archive-cut card (`003-archive-cut.md`) on operator go.
