# Phase 040: God-File Decomposition (Wave 6)

## Goal

Continue reducing high-LOC maintenance hotspots by decomposing the next server/core/TUI candidates into thin orchestration modules with explicit boundaries.

## Scope

Primary Wave 6 targets (current snapshot):
- `src/bin/converge_server/persistence.rs` (~381 LOC)
- `src/remote/operations.rs` (~380 LOC)
- `src/tui_shell/app/cmd_gate_graph.rs` (~373 LOC)

Secondary follow-ons:
- `src/tui_shell/app/cmd_mode_actions.rs` (~365 LOC)
- `src/remote/identity.rs` (~347 LOC)

## Non-Goals

- behavior or UX changes beyond decomposition-safe refactors
- protocol or persistence format redesign
- unrelated performance changes

## Tasks

### A) Baseline and Boundaries

- [x] Capture refreshed target ordering and risk notes.
- [x] Define module boundaries for server persistence, remote operation orchestration, and gate-graph commands.

Progress notes:
- Order/risk:
  - Start with `converge_server/persistence.rs` (mostly pure file/load/default/backfill helpers; low behavior risk).
  - Then `remote/operations.rs` (core flow fan-in; medium risk).
  - Then `cmd_gate_graph.rs` (TUI command semantics/validation; medium risk).
- Boundary intent:
  - Split persistence into path/write helpers, defaults/backfills, and on-disk loaders.
  - Keep top-level modules as orchestration/re-export layers.
  - Prefer `pub(super)`/crate-private visibility without widening APIs.

### B) Server Persistence Decomposition

- [x] Split `src/bin/converge_server/persistence.rs` into focused modules.
- [x] Preserve call-site behavior and existing helper names used by server handlers.
- [x] Reduce wildcard re-export scope where practical after split.

Progress notes:
- Replaced monolithic file with module directory:
  - `src/bin/converge_server/persistence/mod.rs`
  - `src/bin/converge_server/persistence/io_paths.rs`
  - `src/bin/converge_server/persistence/defaults_backfill.rs`
  - `src/bin/converge_server/persistence/repo_load.rs`
- Updated server entry composition to load persistence from `src/bin/converge_server/persistence/mod.rs`.
- Kept externally-used helper names stable (`persist_repo`, `repo_data_dir`, `load_bundle_from_disk`, `write_*`, `load_repos_from_disk`) for handler/runtime call sites.
- Narrowed persistence re-exports to only externally-consumed helpers, leaving internal loaders/backfills module-private.

### C) Remote Operations Decomposition

- [ ] Split `src/remote/operations.rs` by operation families and shared request/execution utilities.
- [ ] Keep public client-facing behavior and error text stable.
- [ ] Add focused unit coverage for newly extracted pure helpers.

### D) TUI Gate Graph Command Decomposition

- [ ] Split `src/tui_shell/app/cmd_gate_graph.rs` into parsing/validation/apply helpers.
- [ ] Keep command UX and output strings behavior-compatible.
- [ ] Reduce cross-module visibility where extraction allows.

### E) Verification and Hygiene

- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.
- [x] Update roadmap notes/checkboxes to match delivered boundaries.

## Exit Criteria

- Wave 6 primary targets are decomposed into thin orchestration layers plus focused helpers.
- Call-site behavior is unchanged and validations pass.
- Roadmap documents actual delivered module boundaries.
