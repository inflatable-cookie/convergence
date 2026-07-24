# 025 Provenance Tightening

Status: complete
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

## Outcome

- `PublishRequest` carries the full `SnapRecord`; server identity-verifies
  and stores it on publish (tampered records rejected — tested);
  `PublicationRecord` gains `snap_parents`
- `GET /api/bundles/:id/provenance` returns the bundle + its input
  publications; CLI `bundle <id>` prints the chain (lane, publisher,
  base, lineage links)
- invariant proven in tests: every superposition variant source in a
  bundle is a registered lane; provenance answers
  who/where-from/on-what for every input
- 76 workspace tests green

## Next Task

Close roadmap `g02.007`; open `g02.008` (releases, retention, GC).
