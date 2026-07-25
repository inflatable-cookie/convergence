# 002 External Backends Locally

Status: active
Updated: 2026-07-25
Roadmap: `g02.018` Batch 18.4

Running the conformance suite against live Postgres and MinIO, on your
machine. The same suite runs nightly in CI (`.github/workflows/backends.yml`).

## Two commands

```bash
effigy backends        # start the services, then run conformance
effigy backends:down   # stop them
```

`effigy backends` is `backends:up` then `backends:test`. Run them
separately when you want the services to stay up between runs:

```bash
effigy backends:up
effigy backends:test
effigy backends:test -- --nocapture
```

## What is running

Effigy's catalog services, declared in `effigy.toml` under
`[containers.backends]` — no repo-owned compose file, and the generated
compose lives under `.effigy/runtime/compose/` as runtime output.

| Service | Image | Port | Credentials |
| --- | --- | --- | --- |
| `postgres` | `postgres:16-alpine` | 5432 | `postgres` / `converge`, database `converge_test` |
| `minio` | `minio/minio` | 9000 (console 9001) | `minioadmin` / `minioadmin` |

The driver is `colima`, so the containers need Colima running.

## The env the tests read

`backends:test` sets these; export them yourself if you run `cargo`
directly:

```bash
CONVERGE_TEST_POSTGRES_URL=postgres://postgres:converge@127.0.0.1:5432/converge_test
CONVERGE_TEST_S3_ENDPOINT=http://127.0.0.1:9000
CONVERGE_TEST_S3_BUCKET=converge-test
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
CONVERGE_REQUIRE_BACKENDS=1
```

`CONVERGE_REQUIRE_BACKENDS=1` is the important one. Without it, an unset
url makes the backend tests print "skipping" and pass — which is how
"external backends are conformance-gated" and "they have never run
against a live service" stayed true at the same time for four roadmaps.
With it, a missing service is a failure.

The MinIO bucket must exist before the S3 tests run:

```bash
docker run --rm --network host --entrypoint sh minio/mc:latest -c \
  "mc alias set local http://127.0.0.1:9000 minioadmin minioadmin && \
   mc mb --ignore-existing local/converge-test"
```

## When it fails

A conformance failure here means a backend disagrees with SQLite or the
local filesystem about behaviour the server depends on — most likely
transaction conflict detection in `apply_batch`, which is the one place
the two metadata stores implement the same contract differently. Treat
it as a product bug in `meta_postgres.rs` or `object_s3.rs`, not as test
flake.

## Next Task

Roadmap `g02.018` is complete; the audit-hardening program `g02.011`-
`g02.018` is closed.
