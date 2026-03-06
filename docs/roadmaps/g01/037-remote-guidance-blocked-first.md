# Phase 037: Remote Guidance (Blocked-First)

## Goal

Make the remote dashboard feel operationally obvious by prioritizing the next actionable work differently per workflow profile.

## Scope

In scope:
- Profile-aware ordering of remote "Next" actions.
- Blocked-first prioritization for game-asset style pipelines.
- Keep current semantics; improve guidance only.

Out of scope:
- New remote APIs or policy model changes.
- Replacing existing inbox/bundles/release behavior.

## Tasks

### A) Next-action prioritization
- [x] Implement profile-aware ordering for dashboard next actions.
- [x] Prioritize blocked constraints first for `game-assets` profile.
- [x] Keep software profile behavior close to current defaults.
- [x] Keep DAW profile focused on inbox -> promotion -> release readiness flow.
- [x] Add direct command-path hints to each suggested action (for example: `[bundles -> approve]`).
- [x] Add focused tests for profile-based action ordering.
- [x] Add a compact remote terminology glossary in the dashboard (`publish`, `bundle`, `promote`, `release`) with profile-aware release wording.

### B) Verification
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
