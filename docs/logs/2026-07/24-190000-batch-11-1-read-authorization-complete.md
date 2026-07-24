# 2026-07-24 Batch 11.1 Complete — Read Authorization

Audit C1 (cross-repo disclosure through unauthorized reads) is closed;
card 038, roadmap `g02.011`, spec 011.

## What landed

- `object_repos` association table in both metadata backends; three new
  `MetadataStore` methods (`associate_object`, `object_in_repo`,
  `remove_object_associations`) with conformance coverage
- `AssociatingObjects` repo-scoped `ObjectStore` wrapper: every
  server-side write (client uploads, publish/verify merge outputs)
  records the association; `has` answers per-repo
- object + negotiate routes moved under `/api/repos/{repo}/…`, gated by
  `read`/`publish`; object reads 404 without the association; negotiate
  reports present-but-unassociated as missing so an idempotent re-put
  repairs the row while dedup still skips byte transfer (doc 16 §1d)
- bundle-id-keyed reads (`get_bundle`, provenance, verify) resolve the
  bundle's repo and require `read` there; unauthorized == absent == 404,
  so bundle ids are not an existence oracle; verify's replay writes are
  scoped to the bundle's repo pending 11.3
- GC sweep drops associations alongside swept objects
- client repo-scopes negotiate/upload/fetch/batch paths; CLI fetch
  passes the configured repo

## Validation

`effigy validate` green (98 tests, incl. two new cross-repo regression
tests); `effigy qa:docs` green; feature clippy
(`backend-postgres,backend-s3`) clean.

## Next

Batch card 11.2: reserve `personal/*`, `snap-sync` capability,
`add_lane_member` inside `authorize`.
