# 2026-07-23 21:15:00 BST - Batch 3.3 CLI Verb Surface Complete

Roadmap: `g02.003`

## Summary

`converge` has its canonical verb surface: local verbs with stable argv,
JSON envelopes, and integration tests driving the compiled binary.

## Changes

- `converge-cli`: `init`, `snap -m`, `history`, `restore --force`,
  `diff`, `resolve list|validate|apply`; global `--json` emitting
  `{ok, data}` / `{ok, error}`; exit codes 0/1/2
- integration tests: local round-trip (init → snap ×2 → history → diff →
  restore), error envelope + exit codes, superposition resolution flow
  through the binary
- opened ready card `009-server-slice.md`

## Validation

- `effigy validate` — fmt, clippy -D warnings, 17 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.003` Batch 3.4 server-slice card.
