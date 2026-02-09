# Phase 044: God-File Decomposition (Wave 10)

## Goal

Continue reducing remote and server hotspots by decomposing transport/fetch and gate handler modules into focused files with unchanged behavior.

## Scope

Primary Wave 10 targets:
- `src/remote/fetch.rs` (~317 LOC)
- `src/remote/transfer.rs` (~302 LOC)
- `src/bin/converge_server/handlers_release.rs` (~311 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries.

Progress notes:
- Start with `remote/fetch.rs` (request/restore/list concerns are separable).
- Follow with `remote/transfer.rs` then `handlers_release.rs`.

### B) Remote Fetch Decomposition
- [x] Split `src/remote/fetch.rs` by operation concern (fetch, restore, release/list helpers).
- [x] Preserve API signatures and fetch/restore behavior.

Progress notes:
- Replaced `src/remote/fetch.rs` with module directory:
  - `src/remote/fetch/mod.rs`
  - `src/remote/fetch/manifest_tree.rs`
  - `src/remote/fetch/object_graph.rs`
- Kept `RemoteClient` fetch entry points and `transfer.rs` helper imports stable via `pub(super)` re-exports from `fetch/mod.rs`.

### C) Remote Transfer Decomposition
- [x] Split `src/remote/transfer.rs` into upload/download concerns.
- [x] Preserve integrity checks and progress reporting behavior.

Progress notes:
- Replaced `src/remote/transfer.rs` with module directory:
  - `src/remote/transfer/mod.rs`
  - `src/remote/transfer/upload.rs`
  - `src/remote/transfer/publish.rs`
- Kept `RemoteClient` public transfer APIs unchanged (`publish_*`, `upload_snap_objects`, `sync_snap`) with behavior-preserving method moves.

### D) Server Release Handler Decomposition
- [x] Split `src/bin/converge_server/handlers_release.rs` by endpoint concern.
- [x] Preserve route signatures and response payloads.

Progress notes:
- Replaced `src/bin/converge_server/handlers_release.rs` with module directory:
  - `src/bin/converge_server/handlers_release/mod.rs`
  - `src/bin/converge_server/handlers_release/release_endpoints.rs`
  - `src/bin/converge_server/handlers_release/promotion_endpoints.rs`
  - `src/bin/converge_server/handlers_release/promotion_state.rs`
- Updated server entrypoint module path in `src/bin/converge-server.rs` to `handlers_release/mod.rs`.

### E) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run`.
- [x] Keep this phase doc updated as slices land.

Progress notes:
- Validation for remote/fetch slice:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo nextest run remote::operations::repo_gate::tests::format_validation_error_limits_issue_lines` passed
  - `cargo test --lib` passed (`15 passed`, `0 failed`)
- Validation for remote/transfer slice:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - Full `cargo nextest run` intermittently stalls after compile in this environment.
  - Fallback targeted validation passed:
    - `cargo test remote_client_modules_compose_across_core_flows -- --nocapture`
    - `cargo test upload_integrity -- --nocapture`
- Validation for handlers_release slice:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - Targeted release/promotion filters passed:
    - `cargo test server_releases -- --nocapture`
    - `cargo test release_permissions -- --nocapture`
    - `cargo test promotion_mechanics -- --nocapture`
