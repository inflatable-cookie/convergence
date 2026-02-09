# Phase 107: God-File Decomposition (Wave 73)

## Goal

Decompose gate handlers, remote admin command handling, and bundle-creation helper logic into smaller modules.

## Scope

Primary Wave 73 targets:
- `src/bin/converge_server/handlers_gates.rs` (~107 LOC)
- `src/cli_exec/remote_admin/remote_ops.rs` (~107 LOC)
- `src/bin/converge_server/handlers_publications/bundles/create_list_get/create.rs` (~125 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Decomposition
- [x] Split `src/bin/converge_server/handlers_gates.rs` into gate-graph and scope modules.
- [x] Split `src/cli_exec/remote_admin/remote_ops.rs` into show/set, create-repo, and purge modules.
- [x] Extract request validation/normalization and id/timestamp helpers from bundle create handler.
- [x] Preserve route behavior and remote/admin CLI output semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- `cargo fmt` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --lib` passed.
- `cargo nextest run` still stalls in this environment after build completes; fallback library tests are green.
