# 2026-07-24 02:00:00 BST - Batch 4.3 TUI Wizards and Resolution Complete

Roadmap: `g02.004`

## Summary

Wizards with back-step, review, and structured-choice rejection; the
superposition resolution view drives decide -> apply through the CLI layer.

## Changes

- `wizard.rs` framework + login/publish wizards (argv-only output)
- resolution view with live undecided counter and decisions-file apply
- reducer + wizard tests; 39 workspace tests

## Validation

- `effigy validate` — green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.004` Batch 4.4 agent-trace card.
