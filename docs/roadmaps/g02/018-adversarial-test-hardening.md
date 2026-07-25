# 018 Adversarial Test Hardening

Status: in progress (18.1-18.2 complete)
Owner: repo maintainers
Updated: 2026-07-25

## Context

The suite (96 tests at audit time) covers happy paths and the merge
decision table well but contains zero concurrency tests, zero failure
injection, zero large-tree cases, and zero pathological-filename cases
— gaps that let every audited race and traversal bug ship. Earlier
roadmaps add regression tests for their own fixes; this roadmap builds
the standing adversarial infrastructure so the next generation of bugs
is caught by CI, not by audit.

## Findings Addressed

- No multi-writer tests: concurrent publishes/promotes to one
  partition never exercised
- No failure injection: kill-mid-GC, kill-mid-export, dropped
  connection mid-batch, corrupted object exercising verify-on-read
- No large-tree/deep-tree/large-window tests; manifest >4096 entries
  (doc 16 §1b deferral) unexercised
- Property tests exist only for chunking — merge determinism,
  variant-key stability, lineage identity are example-based only
- No unicode/pathological filename coverage (would have caught the
  fast-import newline bug)
- No watch-loop timing/ignore test
- External backends never run against live Postgres/MinIO in CI

## Execution Plan (batch details in cards)

- **18.1 Concurrency harness** (complete, card 065): `Cluster` rig with
  N real clients over HTTP; publishes racing promotions, simultaneous
  promotion of one bundle, GC looping against in-flight chunked
  uploads. Found and fixed a duplicate-promotion record — promote is
  now idempotent for retries (doc 14 §3)
- **18.2 Failure injection** (complete, card 066): severed-socket proxy,
  delete-poisoning object store, server-side corruption, and process
  kills during restore and git export. Found two defects — corruption
  reported as 404 while negotiate claimed the object existed, and a
  killed restore leaving staging debris the next snap would capture
- **18.3 Property and pathological input**: proptest coverage for
  merge determinism, variant-key order independence, lineage identity;
  filename fuzzing (unicode, newlines, quotes, near-collisions)
  through capture → merge → git export
- **18.4 Live backend CI lane**: docker-compose Postgres + MinIO
  conformance job; documented local invocation; watch cadence and
  `.convergeignore` tests

## Exit Criteria

- CI exercises at least one true multi-writer interleaving and one
  kill-based failure injection per subsystem (sync, GC, export,
  restore)
- property suites run in default `effigy test`
- external-backend conformance runs green against live services

## Next Task

Open batch card 18.3 (property and pathological input).
