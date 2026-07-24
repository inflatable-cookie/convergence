# 010 Scale and Transport

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

Opens after `g02.009`.

## Context

The slice's simplifications that won't survive scale, staged deliberately
last because none of them change product semantics.

## Planned Batches

- **10.1 Canonical binary encoding**: manifests/records move from JSON to a
  canonical binary form with a stable hashing encoding; chunked manifests
  for very large directories
- **10.2 Batched transport**: negotiate + upload in batched streams instead
  of per-object PUTs; resumable batch sessions
- **10.3 Event push**: server event stream (bundle status, inbox, lane
  activity); TUI subscribes instead of polling
- **10.4 External backends**: Postgres `MetadataStore` and S3-compatible
  `ObjectStore` implementations behind the existing traits; deployment docs

## Exit Criteria

- large-tree benchmarks demonstrate the wins; embedded deployments remain
  the default and unchanged

## Next Task

Compile into batches with a ready card when `g02.009` closes.
