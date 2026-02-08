# Phase 035: Post-Decomposition Hardening

## Goal

Stabilize and harden the decomposed module layout from Phase 034 so boundaries are explicit, behavior is easy to verify, and future changes can be made with lower risk.

## Scope

This phase is limited to follow-through improvements after the god-file decomposition:
- boundary and ownership clarity
- focused regression coverage around extracted flows
- consistency cleanup where decomposition introduced minor structural drift

## Non-Goals

- new product features
- API behavior redesign
- protocol changes between CLI/TUI/server

## Tasks

### A) Boundary Hardening

- [x] Audit visibility in new modules and tighten to the minimum needed (`pub` -> `pub(super)`/private where possible).
- [x] Add short module-level comments/doc headers for non-obvious modules in `src/bin/converge_server/*` and `src/remote/*`.
- [ ] Eliminate any remaining wildcard imports introduced during decomposition if they obscure ownership.

Progress notes:
- Added module headers to non-obvious extracted modules in `src/remote/*` and `src/bin/converge_server/*`.
- Tightened one extracted helper visibility (`fetch_manifest_tree`) and removed one wildcard import coupling between `remote.rs` and `remote/fetch.rs`.

### B) Regression Coverage

- [ ] Add focused tests for remote module composition boundaries (identity/operations/transfer/fetch) where current coverage is indirect.
- [ ] Add a smoke test for CLI command surface/help output stability.
- [ ] Add a server routing smoke test to ensure extracted route registration still wires expected endpoints.

### C) Consistency Cleanup

- [ ] Normalize naming conventions across extracted modules (for example handler/request DTO naming consistency).
- [ ] Ensure each extracted module has a clear single responsibility and move stragglers if needed.
- [ ] Update architecture docs if module ownership changes during this phase.

### D) Verification

- [ ] Run `cargo fmt`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run`.

## Exit Criteria

- Decomposed modules have explicit, minimal visibility boundaries.
- Extracted remote/server/CLI module boundaries are covered by focused regression tests.
- Naming and ownership are consistent enough for new contributors to navigate without historical context.
- Full verification suite passes.
