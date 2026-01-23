# Phase 008: Stable Resolution Variant Keys

## Goal

Make resolution decisions stable across re-coalescing and minor server/client changes by replacing `variant_index` with a stable `variant_key`.

## Motivation

The MVP resolution format stores `path -> variant_index`, which can break if the variant ordering changes.

## Scope

In scope:
- Define a stable `VariantKey` for superposition variants.
- Upgrade the resolution file schema to use `VariantKey`.
- Keep backwards compatibility by supporting reading v1 (index-based) and writing v2 (key-based).
- Update CLI + TUI to use variant keys.

Out of scope:
- Text merges.
- Multi-path macros.

## Proposed VariantKey

A variant is uniquely identified by:
- `source` (publication id)
- `kind` discriminator
- the referenced object identity:
  - file: `blob` + `mode` + `size`
  - dir: `manifest`
  - symlink: `target`
  - tombstone: literal

## Tasks

### A) Model + serialization

- [x] Add `VariantKey` to `src/model.rs` and implement conversion from `SuperpositionVariant`.
- [x] Add `Resolution` v2:
  - decisions: `path -> VariantKey` (plus compatibility for v1 index decisions)
  - keep v1 support for reading

### B) Apply logic

- [x] Update `apply_resolution` to resolve by `VariantKey` (with good error messages).
- [x] Keep a helper that can map `VariantKey -> index` given current variants.

### C) UX updates

- [x] CLI: `resolve pick` accepts either `--variant 1` (convenience) or `--key <json>`.
- [x] TUI: still uses number keys, but stores the chosen variant by key.
- [x] CLI: `resolve show` prints variant keys for copy/paste.

### D) Tests

- [x] Ensure resolution continues to work after deliberately reordering variants.

## Exit Criteria

- Resolutions remain valid if variant list ordering changes.
