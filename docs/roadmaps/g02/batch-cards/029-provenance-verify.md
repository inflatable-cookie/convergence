# 029 Provenance Verify

Status: ready
Updated: 2026-07-24
Roadmap: `g02.008`
Spec: `docs/specs/008-releases-retention-and-gc.md`

## Objective

The determinism feature (vision): replay a bundle from its recorded
provenance and prove the hash — the audit story.

## In Scope

- engine `verify(bundle_id)`: reload the bundle's input publications from
  provenance, re-run the merge fold with the recorded W/strategy/window
  order, recompute the bundle id, compare root manifest + id;
  `VerifyReport { verified, recomputed_root, recorded_root, detail }`
- HTTP `GET /api/bundles/:id/verify`; client + CLI `verify <bundle>`
- tamper detection test: corrupt a stored input-side blob path (swap a
  publication's recorded root in metadata) and verify fails loudly;
  honest bundles verify across all e2e flows incl. text-line-merge
- surface in `bundle <id>` human output (verified badge when asked)

## Out Of Scope

- signing/attestation (future identity roadmap)

## Acceptance Criteria

- roadmap exit criterion: `verify` proves a bundle from provenance; a
  tampered record fails; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- provenance lacks data needed for replay — extend deliberately, doc 17
  first if semantic

## Next Task

On completion, close roadmap `g02.008` against its exit criteria.
