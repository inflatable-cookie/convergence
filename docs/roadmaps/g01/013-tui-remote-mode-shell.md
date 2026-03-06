# Phase 013: TUI Remote Mode (Shell Commands)

## Goal

Expose remote operations through the same command shell without compromising the local-first experience.

Remote mode should be optional: if remote config is missing or server is down, the TUI should still run and show actionable errors.

## Scope

In scope:
- Remote commands executed from the shell.
- Lazy HTTP fetching (only when commands run).
- Minimal text output to scrollback (tables / summaries).

Out of scope:
- Rich list/detail remote views (Phase 014).
- Server-side resolution objects.

## Tasks

### A) Remote config + connectivity

- [x] `remote show`: print configured remote.
- [x] `remote set ...`: optional (if already implemented in CLI, can call through).
- [x] `ping` (or `remote ping`): call `/healthz` and show latency/status.

### B) Remote operations

- [x] `publish [--snap-id ...] [--scope ...] [--gate ...] [--json]`.
- [x] `fetch [--snap-id ...] [--json]`.
- [x] `inbox [--scope ...] [--gate ...] [--limit N]`: list publications.
- [x] `bundles [--scope ...] [--gate ...] [--limit N]`: list bundles.
- [x] `bundle [--scope ...] [--gate ...] [--publication <id>...] [--json]`.
- [x] `approve --bundle-id <id>`.
- [x] `promote --bundle-id <id> --to-gate <id>`.

### C) Superpositions (remote-backed)

- [x] `superpositions --bundle-id <id>`:
  - fetch bundle root manifest tree
  - print conflicted paths + decision status

## Exit Criteria

- All core remote actions are accessible from the TUI shell.
- Remote connectivity issues do not break the TUI; errors appear in scrollback.
