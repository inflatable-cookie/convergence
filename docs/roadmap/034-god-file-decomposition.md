# Phase 034: God-File Decomposition

## Goal

Break up the highest-risk god files into smaller, discoverable modules while preserving behavior.

Primary targets:
- `src/tui_shell/app.rs`
- `src/bin/converge-server.rs`
- `src/main.rs`
- `src/remote.rs`

## Scope

This phase is limited to internal module decomposition and wiring.
No intended product/UX changes beyond tiny fixes needed to preserve existing behavior.

## Non-Goals

- Re-designing workflows, permissions, or API semantics.
- Large feature additions unrelated to decomposition.
- Rewriting tests purely for style.

## Tasks

### A) Decomposition Plan + Guardrails

- [x] Capture file-level decomposition maps (what moves where) for each target file.
- [x] Define module naming conventions and visibility boundaries (`pub`, `pub(crate)`, private helpers).
- [ ] Add/update short module READMEs where needed so entry points are obvious.

Initial decomposition maps:
- `src/tui_shell/app.rs`:
  keep `App` state + high-level dispatch in `app.rs`; move command groups into `src/tui_shell/app/cmd_*.rs` (local/snaps first, then remote/auth/gates/releases/superpositions); later split event-loop and draw helpers into `event.rs`/`render.rs`.
- `src/bin/converge-server.rs`:
  keep `Args`, startup/bootstrap wiring, and top-level router composition in bin entrypoint; move handlers to `src/server/handlers/{identity,repos,lanes,gates,publications,bundles,releases,promotions,objects,gc}.rs`; move persistence/loading into `src/server/persistence/*.rs`; move validation/errors to `src/server/validation.rs` + `src/server/http_error.rs`.
- `src/main.rs`:
  keep CLI type definitions + short `run()` router in `main.rs`; move execution logic into `src/cli/exec/{local,remote,gates,auth,release,promotion,resolution}.rs`; centralize shared argument parsing/utilities in `src/cli/exec/common.rs`.
- `src/remote.rs`:
  keep outward `RemoteClient` API surface stable while moving internals into `src/remote/{types,client,retry,auth,sync,fetch}.rs`; isolate object graph traversal and transfer functions under `sync`/`fetch` modules.

Progress notes:
- Extracted local/snap command handlers to `src/tui_shell/app/cmd_local.rs`.
- Extracted remote root/auth/config command handlers to `src/tui_shell/app/cmd_remote.rs`.
- Extracted remote browsing/membership handlers to `src/tui_shell/app/cmd_remote_views.rs`.
- Extracted remote bundle/promotion/release/superpositions handlers to `src/tui_shell/app/cmd_remote_actions.rs`.
- Extracted gate graph handlers/helpers to `src/tui_shell/app/cmd_gate_graph.rs`.
- Extracted settings handlers/helpers to `src/tui_shell/app/cmd_settings.rs`.
- Extracted publish/sync/fetch-mode transfer handlers to `src/tui_shell/app/cmd_transfer.rs`.
- Extracted remaining mode wrapper handlers to `src/tui_shell/app/cmd_mode_actions.rs`.
- Extracted event loop/key handling to `src/tui_shell/app/event_loop.rs`.
- Extracted rendering helpers to `src/tui_shell/app/render.rs`.
- Extracted superposition navigation helpers to `src/tui_shell/app/superpositions_nav.rs`.
- Extracted timestamp/clock helpers to `src/tui_shell/app/time_utils.rs`.
- Extracted input hint helpers to `src/tui_shell/app/input_hints.rs`.
- Extracted parsing/validation helpers to `src/tui_shell/app/parse_utils.rs`.
- Extracted command dispatch/input pipeline to `src/tui_shell/app/cmd_dispatch.rs`.
- Extracted local maintenance handlers (`show`/`restore`/`move`/`purge`) into `src/tui_shell/app/cmd_local.rs`.
- Extracted settings mutation handlers (`chunking`/`retention`) into `src/tui_shell/app/cmd_settings.rs`.
- Extracted text-input submission/mutation handlers into `src/tui_shell/app/cmd_text_input.rs`.
- Moved inbox/bundles view openers into `src/tui_shell/app/cmd_remote_views.rs`.
- Extracted default action/hint/confirm flow into `src/tui_shell/app/default_actions.rs`.
- Reduced `src/tui_shell/app.rs` to focused orchestration/state (currently ~970 LOC).
- Started `src/main.rs` decomposition with `src/cli_exec.rs` and moved `remote` + `gates` + `token` + `user` + `members` + `lane` command execution branches behind module-level handlers.
- Continued `src/main.rs` decomposition by moving `release` and `resolve` command execution branches into `src/cli_exec.rs`.
- Continued `src/main.rs` decomposition by moving `approve`, `pins`, `pin`, and `status` command execution branches into `src/cli_exec.rs`.
- Continued `src/main.rs` decomposition by moving `publish`, `sync`, and `lanes` command execution branches into `src/cli_exec.rs`.
- Continued `src/main.rs` decomposition by moving `fetch`, `bundle`, and `promote` command execution branches into `src/cli_exec.rs`.
- Continued `src/main.rs` decomposition by moving `login`, `logout`, and `whoami` command execution branches into `src/cli_exec.rs`.
- Continued `src/main.rs` decomposition by moving local command execution branches (`init`, `snap`, `snaps`, `show`, `restore`, `diff`, `mv`) into `src/cli_exec.rs`.
- Collapsed `src/main.rs::run()` routing to `Some(command) => cli_exec::handle_command(command)` so command matching/delegation now lives in `src/cli_exec.rs`.
- Started splitting `src/cli_exec.rs` into submodules with `src/cli_exec/local.rs` for local command runners.
- Continued splitting `src/cli_exec.rs` with `src/cli_exec/identity.rs` for auth/user/member/lane command runners.
- Continued splitting `src/cli_exec.rs` with `src/cli_exec/release_resolve.rs` for release and resolution workflows.
- Continued splitting `src/cli_exec.rs` with:
- `src/cli_exec/remote_admin.rs` for remote config/repo/gate graph handlers.
- `src/cli_exec/delivery.rs` for publish/sync/fetch/bundle/promote/pin/status workflows.
- `src/cli_exec.rs` now acts as thin command router (~190 LOC).
- Started `src/bin/converge-server.rs` split with `src/bin/converge_server/persistence.rs` for repo/state load + persist helpers (no route behavior changes).
- Continued `src/bin/converge-server.rs` split with `src/bin/converge_server/identity_store.rs` for identity timestamp/token/hash/load/persist helpers.
- Continued `src/bin/converge-server.rs` split with `src/bin/converge_server/validators.rs` for shared id/channel/object validation helpers.
- Continued `src/bin/converge-server.rs` split with `src/bin/converge_server/handlers_identity.rs` for identity/user/token HTTP handlers and related request/response DTOs.
- Continued `src/bin/converge-server.rs` split with `src/bin/converge_server/handlers_repo.rs` for repo creation/listing/permissions, repo member management, lane member management, lane listing, and lane head handlers.
- Continued `src/bin/converge-server.rs` split with `src/bin/converge_server/handlers_gates.rs` for gate listing/graph updates and scope creation/listing handlers.

Module conventions (applied in `src/tui_shell/app/*`):
- `cmd_*`: command handlers grouped by domain or interaction surface.
- `event_loop`/`render`: runtime loop and drawing concerns.
- `<topic>_utils` / `<topic>` modules: focused helpers with no command dispatch.
- Visibility defaults to private; use `pub(super)` for cross-submodule `App` methods; reserve `pub(in crate::tui_shell)` only for methods called from sibling modules outside `app/*` (for example `modal.rs`, `wizard.rs`).

### B) Split `src/tui_shell/app.rs`

- [x] Extract command handler groups from `app.rs` into focused modules (for example: `cmd_local`, `cmd_remote`, `cmd_gates`, `cmd_release`, `cmd_resolution`).
- [x] Extract event-loop/key-handling helpers into dedicated modules.
- [x] Extract superposition-specific command and navigation logic into dedicated modules.
- [x] Keep `app.rs` as orchestration/state + high-level dispatch.
- [x] Preserve command names, aliases, and behavior.

### C) Split `src/bin/converge-server.rs`

- [ ] Extract route registration into route-domain modules (identity, repos, lanes, gates, publications/bundles, releases/promotions, objects, GC).
- [ ] Extract handler implementations by domain.
- [ ] Extract persistence/repository-state loading and save helpers into persistence modules.
- [ ] Extract validation and shared response/error helpers into utility modules.
- [ ] Keep a thin `converge-server.rs` entrypoint (`Args`, bootstrap wiring, router composition).

### D) Split `src/main.rs` (CLI)

- [x] Extract command execution logic into domain modules (local, remote, gates, auth, release/promotion, resolution).
- [ ] Keep CLI argument definitions readable and grouped.
- [x] Reduce `run()` match complexity by delegating to module-level executors.
- [ ] Preserve CLI UX and output compatibility.

### E) Split `src/remote.rs`

- [ ] Separate DTO/model definitions from transport/client methods.
- [ ] Extract object-transfer/sync/fetch graph traversal logic into dedicated modules.
- [ ] Extract retry/auth/request helpers into small utility modules.
- [ ] Preserve public `RemoteClient` API behavior used by CLI/TUI.

### F) Verification + Hygiene

- [ ] Keep docs aligned with final structure (`docs/architecture/10-cli-and-tui.md` and relevant READMEs/decision docs as needed).
- [x] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run`.

## Exit Criteria

- `src/tui_shell/app.rs` is substantially reduced and focused on app orchestration rather than command implementation details.
- `src/bin/converge-server.rs` is a thin bootstrap/router entrypoint rather than a single-file server implementation.
- `src/main.rs` command execution is delegated to modules with a significantly simpler `run()`.
- `src/remote.rs` no longer mixes DTOs, transport, and deep sync traversal in one monolithic file.
- Existing tests pass and CLI/TUI/server behavior remains functionally equivalent.
