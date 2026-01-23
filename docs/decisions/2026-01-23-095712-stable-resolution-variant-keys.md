# Decision: Use Stable Variant Keys for Resolution Decisions

Date: 2026-01-23 09:57:12Z

## Context

Resolution MVP stored decisions as `path -> variant_index`.

This is brittle when the variant list order changes (for example due to different coalescing policies or refactors that reorder variants).

## Decision

Resolution decisions are now stored as stable variant keys.

- `Resolution.version = 2` indicates key-based decisions.
- A decision is `path -> VariantKey`.
- `VariantKey` is derived from the variant's content:
  - `source` (publication id)
  - `type` + identity fields:
    - file: `blob`, `mode`, `size`
    - dir: `manifest`
    - symlink: `target`
    - tombstone

Backwards compatibility:
- v1 index-based decisions are still readable.

## Consequences

- Applying a resolution is order-independent within a superposition's variant list.
- Existing v1 resolution files can be upgraded opportunistically when a user picks new decisions.
