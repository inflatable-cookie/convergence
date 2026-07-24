# 034 Canonical Binary Encoding

Status: ready
Updated: 2026-07-24
Roadmap: `g02.010`
Spec: `docs/specs/010-scale-and-transport.md`

## Objective

Manifests, recipes, and snap records move from JSON to a canonical
binary encoding with a stable hashing form; large directories chunk.

## In Scope

- encoding decision recorded in doc 16 first (short amendment): CBOR via
  `ciborium` with canonical map ordering, a 4-byte magic + version
  prefix per object kind; JSON remains the wire/API representation
  (HTTP bodies unchanged) — the *object store* representation changes
- `converge-model` encode/decode helpers; client + server stores use
  them for manifests/recipes/snap records; hashing operates on the
  canonical bytes (ids change — pre-1.0 re-init, no migration)
- manifest chunking for very large directories: entries split into
  sub-manifest pages above N entries (default 4096), transparent to
  readers via a `page` entry kind — doc 16 amendment defines it
- benchmarks (dev-only test, ignored by default) demonstrating encode/
  decode wins on a synthetic 10k-entry tree
- full determinism + e2e suites green under the new encoding

## Out Of Scope

- batched transport (10.2), events (10.3), external backends (10.4)

## Acceptance Criteria

- all suites green; determinism ids stable across fresh stores; doc 16
  amended before code

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- canonicalization ambiguity — doc 16 first

## Next Task

On completion, open the Batch 10.2 batched-transport card.
