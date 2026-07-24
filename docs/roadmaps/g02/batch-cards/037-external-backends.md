# 037 External Backends

Status: ready
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

## Next Task

On completion, close roadmap `g02.010` and the g02.005-g02.010
improvement program.
