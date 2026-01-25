# Phase 027: Retention Tooling Hardening

## Goal

Harden retention/GC ergonomics and contracts so operators can safely prune and garbage-collect without curl and without surprising retention behavior.

## Tasks

### A) CLI regression

- [x] Add an integration test for `converge remote gc --prune-releases-keep-last N`.

### B) API contract

- [x] Extend `tests/server_api_contract.rs` to cover `/repos/:repo_id/gc` auth + response shape.

## Exit Criteria

- `cargo nextest run -P ci` passes.
- The GC endpoint and CLI wrapper are covered by tests.
