# 2026-07-24 01:00:00 BST - Batch 4.1 TUI Shell Core Complete

Roadmap: `g02.004`

## Summary

The rebuilt TUI exists: CLI-as-library argv contract, five-region console
shell, view stack, and the first two UX-spec wart fixes (quit confirm,
named context) — with the key semantics unit-tested as a pure reducer.

## Changes

- `converge-cli` split into lib + thin bin; `execute(argv)` returns the
  JSON payload via the same code path as the binary; new `changes` verb
- `converge-tui`: reducer (`app.rs`) + renderer/runtime (`main.rs`);
  local root and history views over the CLI layer
- 5 reducer tests; workspace tests now 31

## Validation

- `effigy validate` — fmt, clippy -D warnings, 31 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.004` Batch 4.2 remote-views-and-async card.
