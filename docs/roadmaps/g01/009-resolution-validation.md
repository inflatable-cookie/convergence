# Phase 009: Resolution Validation

## Goal

Let users validate a resolution file against the current bundle root manifest without producing a snap.

## Tasks

- [x] Add `converge resolve validate --bundle-id <id>`.
- [x] Implement validation logic: missing decisions, invalid keys, out-of-range indices, extraneous decisions.
- [x] Add tests for validation.

## Exit Criteria

- `converge resolve validate` reports all actionable problems in one run.
