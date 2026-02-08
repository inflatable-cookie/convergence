# CLI and TUI

This document describes UX surfaces and determinism rules.

## CLI principles

- Deterministic by default.
- Stable, scriptable output.
- Prefer a small set of orthogonal verbs.
- Provide `--json` for automation.

Implemented verbs (current):
- `init`, `snap`, `snaps`, `show`, `restore`
- `remote` (configure + `create-repo` dev convenience)
- `publish`, `fetch`
- `bundle`, `approve`, `promote`
- `resolve` (init/pick/clear/show/validate/apply)
- `status`

Planned verbs (not yet implemented):
- `diff`, `release`

## TUI principles

- `converge` (no args) opens an interactive TUI.
- TUI is a client of the same underlying commands/APIs.

TUI capabilities (current):
- Overview: remote config, gate graph, promotion state
- Inbox: publications for configured scope+gate; quick filter; create bundle
- Bundles: list bundles; show promotability + reasons; approve; promote
- Superpositions: inspect conflicts; choose variants; validate/apply resolution (optionally publish)

TUI key bindings (current):
- global: `q`/`esc` quit
- overview: `i` inbox, `b` bundles, `r` reload
- inbox: `space` select, `c` create bundle, `/` filter, `r` refresh
- bundles: `a` approve, `p` promote (with gate chooser if needed), `s` superpositions
- superpositions: `n` next missing, `f` next invalid, `v` validation, `1-9` pick, `0` clear, `a` apply, `p` apply+publish, `r` refresh

## Current code organization

- CLI entrypoint:
  - `src/main.rs` contains argument surface and top-level startup wiring.
  - `src/cli_exec.rs` dispatches command execution.
  - `src/cli_exec/local.rs` handles local snap/store actions.
  - `src/cli_exec/identity.rs` handles auth/user/member/lane actions.
  - `src/cli_exec/remote_admin.rs` handles remote config/repo/gate-graph actions.
  - `src/cli_exec/delivery.rs` handles publish/sync/fetch/bundle/promote/pin/status flows.
  - `src/cli_exec/release_resolve.rs` handles release and resolution workflows.

- Server entrypoint:
  - `src/bin/converge-server.rs` is a thin bootstrap/router composition entrypoint.
  - `src/bin/converge_server/routes.rs` holds authenticated route registration.
  - `src/bin/converge_server/handlers_system.rs` holds auth middleware, healthz, and bootstrap.
  - `src/bin/converge_server/handlers_identity.rs`, `handlers_repo.rs`, `handlers_gates.rs`, `handlers_objects.rs`, `handlers_publications.rs`, `handlers_release.rs`, `handlers_gc.rs` hold domain handlers.
  - `src/bin/converge_server/persistence.rs`, `identity_store.rs`, `validators.rs`, `object_graph.rs`, `access.rs`, `http_error.rs`, `gate_graph_validation.rs` hold shared persistence/validation/error/domain helpers.

- TUI:
  - `src/tui_shell/app.rs` is orchestration/state and delegates behavior to focused modules under `src/tui_shell/app/` (command groups, rendering, event loop, parsing, resolution helpers).

- Remote client:
  - `src/remote.rs` is a thin composition surface for `RemoteClient` construction.
  - `src/remote/types.rs` contains DTO/request types.
  - `src/remote/http_client.rs` contains retry/auth/url/status helpers.
  - `src/remote/identity.rs`, `operations.rs`, `transfer.rs`, `fetch.rs` contain domain operation groups.
  - Ownership boundary rule: extracted `src/remote/*` modules import dependencies explicitly (no wildcard `super::*`), so cross-module coupling is visible at the import site.

- Server decomposition ownership notes:
  - `load_bundle_from_disk` is owned by `src/bin/converge_server/persistence.rs` (disk/state loading concern).
  - GC-only serde default helper (`default_true`) is owned by `src/bin/converge_server/handlers_gc.rs`.
  - Repo/lane membership request payload naming is normalized in `src/bin/converge_server/handlers_repo.rs` via `MemberHandleRequest`.
