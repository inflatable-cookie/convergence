# 029 Provenance Verify

Status: complete
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

## Outcome

- `Engine::verify`: reloads input publications from provenance, replays
  the merge with recorded W/strategy/input order, recomputes the bundle
  hash (now a shared `bundle_hash` fn with the builder), compares root +
  id; `VerifyReport` with human detail
- `GET /api/bundles/:id/verify`; CLI `verify <bundle>` (exit 1 on
  failure — CI-usable)
- tests: honest two-input supersession bundle verifies; tampering a
  recorded publication root in metadata fails verification
- 86 workspace tests green

## Next Task

Close roadmap `g02.008`; open `g02.009` (git interop).
