# 2026-07-23 18:30:00 BST - Batch 2.2 Archive Cut Complete

Roadmap: `g02.002`

## Summary

Archived the g01-era implementation and stripped `main` to the docs spine plus
the salvaged core library.

## Changes

- tagged the full pre-cut state `v0-legacy`; branched `archive/g01` at the
  same commit
- removed from `main`: `src/bin/` (dev server), `src/tui_shell/`, `src/tui.rs`,
  `src/remote*`, `src/cli_*`, `src/main.rs`, `scripts/`, `dev/`, server/CLI/
  e2e tests, unused test helper
- `main` now builds a lib-only crate: `model`, `store`, `diff`, `resolve`,
  `workspace`, under a documented salvage `#![allow(dead_code)]`
- chunking modules retained despite discard verdict — entangled with
  snap/materialize paths; replacement lands with the rebuild (salvage
  inventory unchanged: still discard-at-rebuild)
- Cargo deps pruned to anyhow/blake3/serde/serde_json/time (+tempfile dev)
- effigy.toml: runtime and distribution tasks removed; test task now calls
  nextest directly
- README rewritten for the archived posture
- closed card `003-archive-cut.md`; opened ready card
  `004-docs-spine-restructure.md`; refreshed spec and front doors

## Validation

- `effigy validate` — fmt, check, clippy, 7 nextest tests, all green
- `effigy qa:docs` / `effigy qa:northstar` — green

## Next Task

Execute the `g02.002` Batch 2.3 docs-spine-restructure card.
