# 025 Provenance Tightening

Status: ready
Updated: 2026-07-24
Roadmap: `g02.007`
Spec: `docs/specs/007-lanes-and-collaboration.md`

## Objective

Close the provenance loop: superposition variant sources become verified
lane references, and publication provenance carries the full chain
(lane + publisher + base + snap lineage link).

## In Scope

- publication records already carry lane/publisher/base — add
  `snap_parents` (the published snap's parents) so server-side provenance
  links into lineage without needing the snap record
- publish requires the referenced snap record server-side when available:
  if the snap was synced (7.2), verify `root_manifest` matches the
  record; store the record on publish if absent (client sends it —
  extend `PublishRequest` with the `SnapRecord`)
- superposition variants: source remains the lane id — assert in tests
  that every variant source in a bundle is a registered lane (invariant
  test over the e2e flows)
- `bundle <id>` output gains input publication details (lane, publisher,
  base) — provenance readable from the CLI
- tests: provenance chain assertions across the e2e superposition flow;
  snap-record-on-publish roundtrip; root-manifest mismatch rejection

## Out Of Scope

- provenance replay/verify verb (g02.008 batch 8.4)

## Acceptance Criteria

- a bundle's provenance answers who/where-from/on-what for every input;
  variant sources are registered lanes; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- provenance shape changes ripple into doc 17 — doc first

## Next Task

On completion, close roadmap `g02.007` against its exit criteria.
