# Phase 042: God-File Decomposition (Wave 8)

## Goal

Continue reducing remaining high-LOC files by decomposing handler and wizard hotspots into focused modules while preserving behavior.

## Scope

Primary Wave 8 targets (current snapshot):
- `src/bin/converge_server/handlers_objects.rs` (~322 LOC)
- `src/tui_shell/wizard/member_flow.rs` (~338 LOC)
- `src/tui_shell/wizard/publish_sync_flow.rs` (~322 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target ordering and decomposition boundaries.

Progress notes:
- Start with `handlers_objects.rs` (object-type handler split with low behavior risk).
- Then wizard flows (`member_flow`, `publish_sync_flow`) where transition/effect split boundaries mirror prior waves.

### B) Server Object Handler Decomposition
- [x] Split `src/bin/converge_server/handlers_objects.rs` into object-family modules.
- [x] Preserve route signatures and response behavior.
- [x] Keep shared query/types in a thin module root.

Progress notes:
- Replaced monolithic file with module directory:
  - `src/bin/converge_server/handlers_objects/mod.rs`
  - `src/bin/converge_server/handlers_objects/blob.rs`
  - `src/bin/converge_server/handlers_objects/manifest.rs`
  - `src/bin/converge_server/handlers_objects/recipe.rs`
  - `src/bin/converge_server/handlers_objects/snap.rs`
- Updated server entry composition to load handlers from `handlers_objects/mod.rs`.

### C) Wizard Flow Decomposition
- [x] Split `src/tui_shell/wizard/member_flow.rs` into state transitions, validation, and side-effect helpers.
- [x] Split `src/tui_shell/wizard/publish_sync_flow.rs` into parse/transition/effect helpers.

Progress notes:
- Replaced `src/tui_shell/wizard/member_flow.rs` with module directory:
  - `src/tui_shell/wizard/member_flow/mod.rs`
  - `src/tui_shell/wizard/member_flow/repo_member.rs`
  - `src/tui_shell/wizard/member_flow/lane_member.rs`
- Preserved wizard prompts, action parsing, and final remote-side effects for repo and lane membership flows.
- Replaced `src/tui_shell/wizard/publish_sync_flow.rs` with module directory:
  - `src/tui_shell/wizard/publish_sync_flow/mod.rs`
  - `src/tui_shell/wizard/publish_sync_flow/publish.rs`
  - `src/tui_shell/wizard/publish_sync_flow/sync.rs`
- Preserved publish/sync wizard prompt flow and command argument assembly (`cmd_publish_impl`/`cmd_sync_impl` inputs).

### D) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo nextest run` (or document fallback).
- [x] Update roadmap notes/checkboxes as slices land.

Progress notes:
- Validation for this slice:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - Targeted `nextest`/`cargo test` invocations for server integration tests were intermittently hanging in this environment after compile; full integration verification remains pending.
- Validation for member-wizard split:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
- Validation for publish/sync wizard split:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo test --lib` passed (`15 passed`, `0 failed`)
  - `nextest`/integration invocations continue to intermittently stall in this environment after compile, so full `nextest` remains pending.
