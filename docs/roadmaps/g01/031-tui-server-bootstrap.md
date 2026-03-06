# Phase 031: TUI Server Bootstrap

Goal: make it easy to bootstrap a brand-new `converge-server` (first admin + token) from the TUI.

Non-goals:
- Starting/stopping the server process from the TUI.
- Full operator UI for user/token management (beyond first-admin bootstrap).

## Tasks

### A) Bootstrap Wizard

- [x] Add a `bootstrap` command in remote root.
- [x] Add a guided wizard that:
  - [x] prompts for server URL + bootstrap token + admin handle (optional display name)
  - [x] calls `POST /bootstrap`
  - [x] stores the returned admin token into `.converge/state.json`
  - [x] writes `.converge/config.json` remote config (repo/scope/gate)

### B) Repo Convenience

- [x] After bootstrapping, optionally ensure the repo exists (create if missing).

## Exit Criteria

- `converge` TUI can bootstrap a fresh server via `POST /bootstrap` and ends in a logged-in state.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
