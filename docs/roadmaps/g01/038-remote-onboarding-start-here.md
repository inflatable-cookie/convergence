# Phase 038: Remote Onboarding ("Start Here")

## Goal

Make first-use remote flow explicit in the dashboard so users can execute the right next step without prior Convergence knowledge.

## Scope

In scope:
- Add a "start here" onboarding hint to remote dashboard Next panel.
- Make onboarding hint state-aware (inbox pending, blocked bundles, promotable bundles, release readiness).
- Keep command paths explicit in hint text.

Out of scope:
- New commands or server endpoints.
- Multi-screen onboarding wizards.

## Tasks

### A) Remote start-here guidance
- [x] Add state-aware onboarding hint line in Next panel.
- [x] Include command-path hints (`[inbox -> bundle]`, `[bundles -> promote]`, etc.).
- [x] Cover empty-state onboarding (`publish -> inbox`) for first-time remote users.
- [x] Increase Next panel height to fit onboarding + glossary + actions.

### B) Verification
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
