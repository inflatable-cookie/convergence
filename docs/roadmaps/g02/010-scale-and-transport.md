# 010 Scale and Transport

Status: active
Owner: repo maintainers
Updated: 2026-07-24



## Context

The slice's simplifications that won't survive scale, staged deliberately
last because none of them change product semantics.

## Execution Plan (batch details in cards)

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

Execute the ready Batch 10.2 card (`batch-cards/035-batched-transport.md`).
