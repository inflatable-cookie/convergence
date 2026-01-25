# Phase 024: Rigorous Testing And Tweaking

## Goal

Do a stabilization pass across the full end-to-end workflow (local -> remote -> releases) with deeper automated coverage, tighter invariants, and small UX tweaks discovered during real use.

## Scope

In scope:
- Add missing API/CLI/TUI contract tests.
- Expand negative/edge-case tests for permissions and retention.
- Improve error messages and guidance where tests reveal ambiguity.

Out of scope:
- Large new primitives (artifact signing, CI runners, OIDC, git export).
- Major architecture refactors.

## Tasks

### A) API contract coverage

- [x] Extend `tests/server_api_contract.rs` to cover releases endpoints.
- [x] Add a server restart persistence test for releases (release survives restart).

### B) Retention and GC

- [x] Add a GC retention test asserting releases keep bundles/snaps/objects.
- [x] Add a test for pruning old releases (if/when implemented).

### C) CLI/TUI regression tests

- [x] Add a CLI e2e test that: publish -> bundle -> release -> fetch --release --restore -> diff vs original tree.
- [x] Add a TUI smoke test plan (manual checklist doc) for: releases view, inbox, bundles, superpositions, lanes.

### D) UX tweaks

- [x] Normalize CLI/TUI help strings around releases, login, and fetch.
- [x] Improve error messages for common permission failures (publish vs read vs admin).

## Exit Criteria

- Releases endpoints are covered by contract tests.
- GC retention behavior is covered by automated tests.
- Core e2e workflows have regression tests (CLI) and a documented manual TUI checklist.
