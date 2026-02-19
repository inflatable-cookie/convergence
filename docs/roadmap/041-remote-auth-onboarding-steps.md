# Phase 041: Remote Auth Onboarding Steps

## Goal

Replace single-line auth hints with explicit step-by-step recovery paths so first-time and broken-state users can recover without prior system knowledge.

## Scope

In scope:
- State-aware onboarding steps for remote auth-required panel.
- Distinct command paths for unauthorized/login-required vs unreachable server vs server error.

Out of scope:
- New auth protocols.
- Changes to server auth behavior.

## Tasks

### A) Auth panel onboarding
- [x] Replace single `hint:` line with short numbered `start here` steps.
- [x] Add tailored steps per auth failure category.
- [x] Keep commands copyable and directly executable from prompt.

### B) Verification
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
