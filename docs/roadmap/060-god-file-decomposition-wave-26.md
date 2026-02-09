# Phase 060: God-File Decomposition (Wave 26)

## Goal

Continue reducing workspace scan complexity by separating on-disk manifest creation and in-memory manifest tree scanning into focused modules with shared filesystem helpers.

## Scope

Primary Wave 26 target:
- `src/workspace/manifest_scan.rs` (~207 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target and decomposition boundaries.

### B) Manifest Scan Decomposition
- [x] Split `src/workspace/manifest_scan.rs` into focused modules for store-backed scan and in-memory scan paths.
- [x] Preserve chunking thresholds, file mode handling, symlink behavior, and stats accounting semantics.

### C) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment stalls persist).
- [x] Keep this phase doc updated as slices land.

## Verification Notes

- 2026-02-09: `cargo fmt` passed.
- 2026-02-09: `cargo clippy --all-targets -- -D warnings` passed.
- 2026-02-09: `cargo nextest run` compiled and then stalled in this environment after build completion; fallback `cargo test --lib` passed (15 passed, 0 failed).
