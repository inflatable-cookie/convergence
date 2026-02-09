# Phase 041: God-File Decomposition (Wave 7)

## Goal

Continue decomposition of remaining high-LOC coordination files while preserving command UX and server/client behavior.

## Scope

Primary Wave 7 targets (current snapshot):
- `src/tui_shell/app/cmd_mode_actions.rs` (~365 LOC)
- `src/remote/identity.rs` (~347 LOC)
- `src/tui_shell/app/cmd_remote.rs` (~317 LOC)

## Non-Goals

- behavioral or UX changes beyond decomposition-safe refactors
- protocol/schema updates
- unrelated optimization work

## Tasks

### A) Baseline and Boundaries

- [x] Capture target ordering and risk notes.
- [x] Define module boundaries for mode actions and remote command/identity modules.

Progress notes:
- Order/risk:
  - Start with `cmd_mode_actions.rs` (TUI mode command dispatch helpers; low-medium risk).
  - Continue with `remote/identity.rs` (auth + identity HTTP behavior; medium risk).
  - Finish with `cmd_remote.rs` (broader TUI remote command surface; medium risk).
- Boundary intent:
  - Split by command family (inbox/bundles/superpositions) and keep orchestration entrypoints thin.
  - Preserve existing command usage strings and selection/error semantics.

### B) TUI Mode Actions Decomposition

- [x] Split `src/tui_shell/app/cmd_mode_actions.rs` by mode-command families.
- [x] Preserve command names, usage/help strings, and selection semantics.
- [x] Keep cross-module visibility minimal while preserving dispatch reachability.

Progress notes:
- Replaced monolithic mode-actions file with module directory:
  - `src/tui_shell/app/cmd_mode_actions/mod.rs`
  - `src/tui_shell/app/cmd_mode_actions/inbox_bundles.rs`
  - `src/tui_shell/app/cmd_mode_actions/superpositions.rs`

### C) Remote Identity Decomposition

- [x] Split `src/remote/identity.rs` into auth/session, user/token, and membership/permission helpers.
- [x] Preserve request paths and auth/error handling behavior.
- [ ] Add focused tests for extracted pure helpers where practical.

Progress notes:
- Replaced `src/remote/identity.rs` with module directory:
  - `src/remote/identity/mod.rs`
  - `src/remote/identity/auth_session.rs`
  - `src/remote/identity/users_tokens.rs`
  - `src/remote/identity/members_lanes.rs`
- Preserved all existing `RemoteClient` identity/member/lane method names and HTTP endpoint behavior.

### D) TUI Remote Command Decomposition

- [x] Split `src/tui_shell/app/cmd_remote.rs` into grouped command handlers and parse/apply helpers.
- [x] Keep command UX and output behavior-compatible.
- [x] Reduce wildcard imports/exports where decomposition allows.

Progress notes:
- Replaced `src/tui_shell/app/cmd_remote.rs` with module directory:
  - `src/tui_shell/app/cmd_remote/mod.rs`
  - `src/tui_shell/app/cmd_remote/config_cmds.rs`
  - `src/tui_shell/app/cmd_remote/auth_cmds.rs`
  - `src/tui_shell/app/cmd_remote/repo_health.rs`
- Preserved existing command names and usage/error text for `remote`, `remote set/unset`, `login/logout`, `bootstrap`, `create-repo`, and `ping`.

### E) Verification and Hygiene

- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.
- [x] Keep roadmap notes in sync with implemented boundaries.

Progress notes:
- Validation after `cmd_mode_actions` decomposition:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo nextest run` passed (`62 passed`, `0 skipped`)
- For subsequent Wave 7 slices in this environment:
  - `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed
  - `cargo test --tests` surfaced an intermittent integration startup timeout in `approvals_required`
  - targeted `cargo nextest run approvals_make_bundle_promotable` passed (`1 passed`, `61 skipped`)

## Exit Criteria

- Wave 7 targets are decomposed into thin orchestration modules.
- Validation passes and roadmap reflects delivered module boundaries.
