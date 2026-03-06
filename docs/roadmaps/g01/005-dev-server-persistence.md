# Phase 005: Dev Server Persistence

## Goal

Make `converge-server` behave like a long-running authority across restarts in dev.

Today the server keeps critical repo state (repos list, publications, bundles index, promotion state, gate graph, scopes, snaps set) only in memory, so restarting the process requires re-creating repos and loses list endpoints.

## Scope

In scope:
- Persist and reload repo state in the dev server data directory (`--data-dir`).
- Ensure list endpoints reflect previously-created objects after restart.

Out of scope:
- Migrations across schema versions.
- Durable auth/identity (still dev-token).

## Tasks

### A) Repo state persistence

- [x] Persist repo state to `repo.json` inside each repo directory.
- [x] Update server mutations to write `repo.json` when state changes:
  - repo creation
  - scope creation
  - gate graph updates
  - snap upload (snaps set)
  - publication creation
  - bundle creation / bundle approval
  - promotions / promotion state

### B) Reload on startup

- [x] On server startup, scan `--data-dir` for repo directories.
- [x] Load `repo.json` when present; otherwise best-effort reconstruct minimal repo state.
- [x] Ensure bundles/promotions indices are hydrated from disk so list endpoints work.

### C) Tests

- [x] Add an integration test that starts the server, creates repo+publication+bundle, restarts the server on the same data dir, and verifies state is still visible.

## Exit Criteria

- Restarting `converge-server` does not require re-creating the repo.
- `GET /repos/:repo/publications` and `GET /repos/:repo/bundles` return the pre-restart data.
