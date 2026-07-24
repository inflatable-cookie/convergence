# 038 Read Authorization

Status: complete
Updated: 2026-07-24
Roadmap: `g02.011`
Spec: `docs/specs/011-server-trust-boundaries.md`

## Objective

Audit C1 closed: no read endpoint serves content without proving the
caller may read the repo that content belongs to.

## In Scope

- `object_repos` association table (repo_id, kind, object_id) in both
  metadata backends + conformance coverage; rows written on every
  server-side object write (client uploads, engine merge outputs) and
  removed by GC sweep
- object routes move repo-scoped:
  `/api/repos/:repo/objects/:kind/:id`, `/api/repos/:repo/objects/
  batch`, `/api/repos/:repo/objects/batch-get`,
  `/api/repos/:repo/negotiate`; handlers `authorize(subject, repo,
  Read)` (writes: `Publish`) and object reads additionally require the
  association row
- bundle-id-keyed reads (`get_bundle`, `get_provenance`,
  `verify_bundle`) resolve the bundle's repo then `authorize(...,
  Read)`
- client `remote.rs` updated to the repo-scoped routes (repo id is
  already in workspace config)
- regression tests: token with read on repo A denied objects/bundles/
  provenance of repo B (404/403), same object uploaded to both repos
  readable from both, negotiate probe denied cross-repo

## Out Of Scope

- `verify` store mutation (11.3), batch caps (11.4), lane namespace
  (11.2), scope registry (14.3)

## Acceptance Criteria

- every route in `http.rs` calls `authorize()` (or is `healthz`);
  cross-repo denial tests green; existing e2e suites green after the
  route migration

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- association shape fights the shared-store dedup model — spec 011
  design pins first

## Outcome

- `object_repos` association table in both metadata backends (three new
  `MetadataStore` methods, conformance-covered); every server-side
  object write goes through the new `AssociatingObjects` repo-scoped
  wrapper (client uploads, publish/verify merge outputs), so membership
  is recorded at write time
- object + negotiate routes moved under `/api/repos/{repo}/…` and
  authorized (`read` for reads, `publish` for writes); object reads 404
  without the association; negotiate reports present-but-unassociated
  as missing so an idempotent re-put repairs the row (doc 16 §1d)
- bundle-id-keyed reads (`get_bundle`, provenance, verify) resolve the
  bundle's repo and require `read` there; unauthorized == absent == 404
  (no cross-repo existence oracle); verify's replay writes are scoped
  to the bundle's own repo pending 11.3
- GC sweep drops associations with swept objects
- client repo-scopes `negotiate`/`upload_tree`/`fetch_bundle` and the
  batch paths; CLI fetch passes the configured repo
- regression tests: cross-repo denial (bundle/provenance/verify/object/
  negotiate, no detail leakage) and dedup-with-two-associations both
  green; 98 workspace tests pass; doc 16 §1c/§1d updated

## Next Task

Batch card 11.2 (namespace and capability integrity).
