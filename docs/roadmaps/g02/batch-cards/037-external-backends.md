# 037 External Backends

Status: complete
Updated: 2026-07-24
Roadmap: `g02.010`
Spec: `docs/specs/010-scale-and-transport.md`

## Objective

The arch-14 deployment promise made real: Postgres `MetadataStore` and
S3-compatible `ObjectStore` behind the existing traits; embedded stays
the default.

## In Scope

- `PostgresMetadataStore` (feature-gated `backend-postgres`): same
  schema shape as SQLite, per-mutation transactions; integration test
  behind an env gate (`CONVERGE_TEST_POSTGRES_URL`) so CI without
  Postgres skips
- `S3ObjectStore` (feature-gated `backend-s3`, S3-compatible):
  put-if-absent via head-then-put, verify-on-read unchanged; env-gated
  integration test (`CONVERGE_TEST_S3_*`) — MinIO-friendly
- server bin: `--metadata <sqlite path | postgres url>` and
  `--objects <fs path | s3 url>` selection; embedded defaults unchanged
- trait-conformance suite shared across backends (embedded always;
  external when env present)
- operator note in doc 14 (config examples)

## Out Of Scope

- managed-service automation, migrations tooling

## Acceptance Criteria

- conformance suite green on embedded; external backends compile behind
  features and pass conformance when env is provided; embedded default
  unchanged

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- trait shape fights a backend — arch first, no backend-specific leaks

## Outcome

- `PostgresMetadataStore` (feature `backend-postgres`) and
  `S3ObjectStore` (feature `backend-s3`, MinIO-friendly path-style)
  behind the existing traits; verify-on-read/write and idempotent-put
  discipline unchanged
- server bin: `--metadata <path | postgres://...>` and
  `--objects <path | s3://bucket?endpoint=...>` selection; embedded
  defaults untouched; clear errors when a URL names a backend the
  binary was built without
- shared conformance suite (metadata + objects behavioral checks):
  embedded always green; external variants env-gated
  (`CONVERGE_TEST_POSTGRES_URL`, `CONVERGE_TEST_S3_*`) and compiled
  behind their features — clippy-clean under
  `--features backend-postgres,backend-s3`
- honest caveat recorded: external impls are compile-checked and
  conformance-gated, not yet run against live Postgres/MinIO in this
  environment — first deployment should run the conformance suite with
  env set
- doc 14 §5c operator note with config examples
- 96 workspace tests green

## Next Task

Close roadmap `g02.010` and the g02.005-g02.010 improvement program.
