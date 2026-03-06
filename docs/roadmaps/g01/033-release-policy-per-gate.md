# Phase 033: Release Policy Per Gate (Remove “Terminal Gate”)

Goal: simplify releases by allowing any gate to create a release by default.

Today the gate graph has a `terminal_gate` concept and some flows assume “release only from terminal”. This phase removes that abstraction and replaces it with a per-gate `allow_releases` policy knob (default `true`).

Non-goals:
- Full release-channel policy system (beyond what exists today).
- Reworking approvals/superpositions semantics.

## Tasks

### A) Schema + Migration

- [x] Remove `terminal_gate` from the gate graph schema.
- [x] Add `allow_releases: bool` to each gate definition (default `true`).
- [x] Add migration when loading existing repos:
  - [x] drop/ignore persisted `terminal_gate` (server accepts legacy payloads)
  - [x] set `allow_releases=true` for all gates (via defaulting)

### B) Server Semantics

- [x] Update gate graph validation to stop requiring `terminal_gate`.
- [x] Keep “reachable from a root gate” validation (still useful) unless there is a strong reason to relax it.
- [x] Update release creation endpoints to:
  - [x] validate gate exists in stored graph
  - [x] reject if `allow_releases == false`
  - [x] remove any special “terminal” checks

### C) Client UX

- [x] CLI:
  - [x] `converge release ...` should work from any gate (unless disabled).
  - [x] `converge gates init` should include `allow_releases` in the scaffold.
- [x] TUI:
  - [x] Remove “set terminal gate” flows.
  - [x] Add `toggle-releases` (or similar) for a gate.
  - [x] Update gate details panel to show `allow_releases`.

### D) Docs + Tests

- [x] Update `docs/architecture/12-gate-graph-schema.md`.
- [x] Update operator docs to describe release eligibility by gate.
- [x] Update/extend tests:
  - [x] Release allowed from non-terminal gates by default.
  - [x] Release rejected when `allow_releases=false`.
  - [x] Back-compat: repos with legacy `terminal_gate` data load/migrate cleanly.

## Exit Criteria

- Releases can be created from any gate by default.
- Gate graph no longer requires `terminal_gate`.
- Per-gate `allow_releases` works end-to-end (server + CLI + TUI) and is persisted.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
