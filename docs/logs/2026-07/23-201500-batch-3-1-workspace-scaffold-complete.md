# 2026-07-23 20:15:00 BST - Batch 3.1 Workspace Scaffold Complete

Roadmap: `g02.003`

## Summary

First rebuild code batch: five-crate Cargo workspace with `converge-model`
as the shared foundation and FastCDC chunking proven by property tests.

## Changes

- workspace: `converge-model`, `converge-client` (salvage + tests migrated),
  `converge-cli` / `converge-tui` / `converge-server` stubs
- `converge-model`: flattened object model, new `wire.rs` DTOs
  (negotiate/publication/bundle/lane-head/gate-graph, `WIRE_VERSION`), new
  `chunk.rs` FastCDC chunker; `FileRecipe` gains optional `params` header
  (absent on g01 v1 recipes)
- five chunking property tests: determinism, exact reassembly, bounded
  insert/delete damage on 16 MiB inputs, params recorded
- workspace deps centralized; `fastcdc` added

## Validation

- `effigy validate` — fmt, clippy -D warnings, 12 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.003` Batch 3.2 client-core card.
