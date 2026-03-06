# Phase 039: Remote Onboarding (Owners And Defaults)

## Goal

Reduce decision friction by showing who should act next and by pre-filling release channels with workflow-profile defaults.

## Scope

In scope:
- Add role/owner cues to dashboard next actions.
- Provide profile-based default release channels in the release wizard.

Out of scope:
- New permission systems or role models.
- Channel policy enforcement changes.

## Tasks

### A) Owner cues in next actions
- [x] Add owner hints to each recommended dashboard action.
- [x] Make owner hints profile-aware (`software`, `daw`, `game-assets`).

### B) Release defaults
- [x] Add profile-based default release channel in release wizard.
- [x] Surface default channel in release prompt help text.

### C) Verification
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
