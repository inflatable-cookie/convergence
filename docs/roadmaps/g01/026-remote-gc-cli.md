# Phase 026: Remote Purge CLI

## Goal

Make server GC and release-pruning usable without curl by adding a first-class CLI wrapper.

## Scope

In scope:
- `converge remote purge` CLI.
- Remote client method to call `/repos/:repo_id/gc`.
- Doc update to reference CLI usage.
- A small CLI e2e test.

Out of scope:
- TUI UI for purge.
- Additional server-side GC policies.

## Tasks

- [x] Add `RemoteClient::gc_repo(...)`.
- [x] Add `converge remote purge` command.
- [x] Update operator docs to prefer the CLI over curl.
- [x] Add CLI e2e test for `remote purge`.

## Exit Criteria

- Operators can run `converge remote purge --prune-releases-keep-last N`.
