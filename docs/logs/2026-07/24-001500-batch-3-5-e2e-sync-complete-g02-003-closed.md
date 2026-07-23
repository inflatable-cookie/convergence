# 2026-07-24 00:15:00 BST - Batch 3.5 E2E Sync Complete; g02.003 Closed

Roadmap: `g02.003`

## Summary

The wire is real: client and server speak the arch-16 contract over HTTP,
and the whole vertical slice — snap, publish, superposition bundle, local
resolve, republish, approve, promote, fetch, materialize — passes as one
end-to-end test against a live server. Roadmap `g02.003` is closed; the TUI
rebuild (`g02.004`) is the next owner with a ready card.

## Changes

- server: axum HTTP surface + dev bin (`--addr/--data-dir/--token/--seed-dev`)
- client: `remote.rs` with Merkle-pruned two-phase negotiation, idempotent
  uploads, bundle fetch; CLI verbs `login`/`publish`/`fetch`/`status`
- model: `ObjectSet` + request DTOs; `ObjectId: Ord`
- e2e assertions: dedup on second publish, re-upload negotiates to zero,
  wire-version refusal, unknown-token 401
- closed card 010, roadmap `g02.003`, spec 003 (archived); opened
  `g02.004` TUI-rebuild roadmap, spec 004, ready card 011

## Validation

- `effigy validate` — fmt, clippy -D warnings, 26 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.004` Batch 4.1 TUI command-layer-and-shell-core card.
