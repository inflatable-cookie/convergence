# 006 Workspace Scaffold and Model

Status: ready
Updated: 2026-07-23
Roadmap: `g02.003`
Spec: `docs/specs/003-rebuild-vertical-slice.md`

## Objective

Stand up the five-crate workspace and make `converge-model` the single shared
foundation: salvaged object model plus wire DTOs plus the new FastCDC recipe
format.

## In Scope

- Cargo workspace per arch 13: `converge-model`, `converge-client`,
  `converge-cli`, `converge-tui` (stub), `converge-server` (stub)
- move `src/model/` into `converge-model`; keep hash/ID discipline intact
- define wire DTOs in `converge-model` (publication, bundle, lane, gate
  graph, negotiation messages per arch 16)
- FastCDC chunker in `converge-model` (or a `converge-chunk` module within
  it): parameters in recipe header, property tests for boundary stability
  under insert/delete edits
- point the remaining salvaged lib modules at the new model crate so the
  tree still builds (full client migration is Batch 3.2)
- retarget `effigy.toml` tasks at the workspace

## Out Of Scope

- store/diff/resolve/workspace migration (Batch 3.2)
- any server logic beyond the crate stub
- CLI verbs

## Acceptance Criteria

- `cargo build` / `effigy validate` green across the workspace
- `converge-model` has no dependency on any other workspace crate
- chunking property tests demonstrate: identical content → identical chunks;
  single edit shifts a bounded number of chunks
- salvaged tests still pass

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- model changes required beyond mechanical moves — route through
  `docs/architecture/` first

## Next Task

On completion, open the Batch 3.2 client-core card.
