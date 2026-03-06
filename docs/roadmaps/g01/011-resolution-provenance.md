# Phase 011: Resolution Provenance

## Goal

When a resolved snap is published, record provenance linking the publication back to the bundle and resolution that produced it.

This makes it possible to answer:
- which bundle was resolved
- who resolved it
- what resolution file / decisions were applied

## Scope

In scope:
- Extend `Publication` to optionally carry `resolution` metadata.
- CLI `converge resolve apply --publish` populates the metadata.
- Server persists and returns the metadata from list/get endpoints.
- Minimal tests.

Out of scope:
- Server-side storage of full resolution files (still local).
- Rich provenance graphs.

## Tasks

### A) Model + API

- [x] Add optional `resolution` field to publication JSON:
  - `bundle_id`
  - `root_manifest`
  - `resolved_root_manifest`
  - `created_at`

### B) CLI

- [x] When publishing from `resolve apply --publish`, attach the `resolution` metadata.

### C) Persistence

- [x] Ensure server persistence (`repo.json`) includes the new publication fields.

### D) Tests

- [x] Add a test that publishes a resolved snap and verifies the publication includes resolution metadata.

## Exit Criteria

- `GET /repos/:repo/publications` includes resolution provenance for resolved publications.
