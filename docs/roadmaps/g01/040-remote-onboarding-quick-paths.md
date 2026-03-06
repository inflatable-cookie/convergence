# Phase 040: Remote Onboarding (Quick Paths)

## Goal

Reduce first-action hesitation by showing explicit command paths for the current remote state directly in the dashboard.

## Scope

In scope:
- Add state-aware "quick path" guidance line in remote Next panel.
- Include profile-aware release-channel defaults in quick-path guidance.
- Keep changes guidance-only (no command semantics changes).

Out of scope:
- New commands or multi-step onboarding wizards.
- Additional backend state.

## Tasks

### A) Quick-path guidance
- [x] Add a dedicated quick-path line after start-here hint in Next panel.
- [x] Ensure quick-path state mapping matches onboarding state transitions.
- [x] Include profile-aware default release channel in quick-path release hint.
- [x] Resize Next panel to fit flow + profile + onboarding + quick path + glossary + actions.

### B) Verification
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
