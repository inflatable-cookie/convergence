# Phase 036: TUI Second-Nature UX Hardening

## Goal

Make the TUI feel natural for first-time and repeat users by reducing setup ambiguity, sharpening action guidance, and aligning docs with the current interaction model.

## Scope

In scope:
- TUI auth/setup diagnostics and next-step hints.
- Context and command discoverability improvements.
- Docs alignment for current TUI bindings and flows.

Out of scope:
- New domain primitives or remote protocol changes.
- Full redesign of view layouts.

## Persona Findings

### 1) First-time local user
- Confusion: core navigation (`Tab`, `/`, default action on `Enter`) is easy to miss.
- Roadblock: hidden mental model for “contexts + command palette”.

### 2) New remote collaborator
- Confusion: generic `auth: error` does not distinguish bad token vs server outage.
- Roadblock: unclear recovery path from auth/setup failures.

### 3) Context-switching operator
- Friction: valid commands can fail in the wrong root context with a hard stop.
- Roadblock: frequent local/remote context switching tax.

### 4) Release/bundle operator
- Friction: differing command mental models between CLI and TUI for some verbs.
- Roadblock: uncertain “which surface is source of truth” for actions.

### 5) Browse-heavy reviewer
- Friction: multi-step edit prompts (`scope -> gate -> filter -> limit`) for simple edits.
- Roadblock: repetitive modal steps for common list changes.

## Usage Mode Expectations

Convergence should adapt guidance to the dominant workflow mode instead of assuming software-code defaults.

### Mode A) DAW / audio production (Loophole-style)
- Primary local activity:
  - frequent snapshots of project file + session state
  - occasional large binary assets (stems, samples, rendered previews)
- Remote expectation:
  - clear separation between work-in-progress assets and release artifact
  - release should read as "mastered mixdown candidate", not generic bundle movement
- UX requirement:
  - remote root should explain publication flow in plain production terms
  - release creation should emphasize channel intent (for example: `master`, `preview`, `internal`)

### Mode B) Game asset pipeline
- Primary local activity:
  - high-volume binary and metadata churn across many contributors
  - branch-like collaboration lanes by discipline (art/audio/design)
- Remote expectation:
  - strong inbox/bundle triage and conflict visibility
  - promotion criteria tied to gate policy (approvals, superpositions, QA gates)
- UX requirement:
  - remote root and list views should surface "what is blocked and why" first
  - lane/bundle/release actions should be discoverable without command memorization

### Mode C) Software code pipeline
- Primary local activity:
  - frequent text diffs with occasional artifact snapshots
- Remote expectation:
  - familiar review/promote/release progression with deterministic automation
- UX requirement:
  - keep CLI parity crisp while TUI remains guided and low-friction

## Tasks

### A) Auth and recovery clarity
- [x] Classify remote auth failures into actionable categories (`unauthorized`, `server unreachable`, `server error`, generic).
- [x] Tailor remote auth-block hints to the detected failure category.
- [x] Keep diagnostics commands (`remote`, `ping`, `refresh`) available when remote is configured but identity is unavailable.
- [x] Adjust remote root default hints to include diagnostics (`login | ping`) when auth is missing.

### B) Interaction discoverability
- [x] Add persistent root-level keystrip for core controls (`/`, `Tab`, `Enter`, `Esc`) rather than relying on log-only onboarding text.
- [x] Add “wrong context” recovery action (auto-switch or one-keystroke confirm to run in target context).

### C) Browse-flow efficiency
- [x] Replace stepwise inbox/bundles `edit` wizard with a compact single-form editor (scope/gate/filter/limit).

### D) Surface and docs consistency
- [x] Update architecture doc keybinding section to match current TUI behavior.
- [x] Add a CLI/TUI command parity note in docs so differences are explicit and intentional.

### E) Mode-adaptive remote guidance
- [x] Add a lightweight remote onboarding panel explaining `publish -> bundle -> promote -> release` in plain language.
- [x] Add optional workflow profile presets (for example: `software`, `daw`, `game-assets`) that tune default hints and labels only.
- [x] Ensure release prompts can display profile-sensitive guidance text (for example: mastered mixdown criteria for DAW profile).

### F) Validation
- [x] Run `cargo fmt`.
- [x] Run `cargo check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.

## Verification Notes

- `cargo fmt` passed.
- `cargo check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
