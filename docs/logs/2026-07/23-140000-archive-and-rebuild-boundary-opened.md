# 2026-07-23 14:00:00 BST - Archive-and-Rebuild Boundary Opened

Roadmap: `g02.002`

## Summary

Resolved the `g02.001` next-boundary decision and opened `g02.002` as the
archive-and-rebuild owner.

Operator intent: concept validated, implementation archived. Full-repo
assessment (docs + code surveys) established the salvage verdicts recorded in
`docs/specs/002-archive-and-rebuild-boundary.md`: carry the content-addressed
model (incl. superposition-as-data), local store, diff/resolve, and sync
protocol shape; discard the dev-server storage layer, fixed-block chunking, and
TUI implementation (UX survives as a captured spec). Rebuild lands as
independent client and server systems in one Cargo workspace with a shared
model crate. Archive mechanics: `v0-legacy` tag + `archive/g01` branch, then
strip `main`.

## Changes

- closed `g02.001` (Batch 1.2 decision made)
- opened `g02.002` roadmap and spec `002-archive-and-rebuild-boundary.md`
- opened ready card `002-capture-lessons-tui-ux-and-salvage-inventory.md`
- archived spec `001-post-research-next-boundary-gate.md`
- realigned front doors: docs README, roadmaps README, g02 README,
  generation index, specs README, working-rules Next Task

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Next Task

Execute the ready `g02.002` Batch 2.1 capture card.
