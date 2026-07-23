# 2026-07-23 18:00:00 BST - Batch 2.1 Capture Complete

Roadmap: `g02.002`

## Summary

Completed the Batch 2.1 capture card: everything worth keeping from the
g01-era implementation is now written down ahead of the archive cut.

## Changes

- added `docs/rebuild/001-lessons-retrospective.md` — validated theory
  (cheap snaps, publish-time gate, superposition-as-data), failure lessons
  (dev-fixture server vs large-org claim, TUI implementation sprawl, docs
  process weight), open questions feeding Batch 2.4
- added `docs/rebuild/002-tui-ux-spec.md` — implementation-independent UX
  spec from a full `tui_shell/` survey: console-hybrid shell model, ten
  views, key semantics, twelve wizard flows, eight preserved interaction
  principles (incl. TUI-as-thin-front-end-over-CLI-argv and the JSONL agent
  trace), seven warts the rebuild must fix
- added `docs/rebuild/003-salvage-inventory.md` — concrete carry/discard
  paths for the cut
- closed batch card `002-capture-lessons-tui-ux-and-salvage-inventory.md`
- opened ready card `003-archive-cut.md`, gated on explicit operator go
- refreshed spec `002`, roadmap `g02.002`, and front doors

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Next Task

Execute the `g02.002` Batch 2.2 archive-cut card on operator go.
