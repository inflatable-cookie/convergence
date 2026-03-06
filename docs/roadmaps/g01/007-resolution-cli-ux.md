# Phase 007: Resolution CLI UX

## Goal

Make superposition resolution usable without the TUI by providing CLI commands to create, edit, and inspect resolution files.

## Scope

In scope:
- `converge resolve init` to create `.converge/resolutions/<bundle_id>.json`.
- `converge resolve pick/clear/show` to manage per-path decisions.
- Validate pick operations against the bundle's current root manifest (path exists + variant index in range).

Out of scope:
- Stable variant keys (beyond index) across re-coalescing.

## Tasks

### A) CLI commands

- [x] Add `converge resolve init --bundle-id <id>`.
- [x] Add `converge resolve pick --bundle-id <id> --path <path> --variant <n>`.
- [x] Add `converge resolve clear --bundle-id <id> --path <path>`.
- [x] Add `converge resolve show --bundle-id <id>`.

### B) Validation helpers

- [x] Add shared helpers to enumerate superpositions in a manifest tree and return per-path variant counts.

### C) Tests

- [x] Extend/adjust the Phase 6 e2e test to use the CLI commands (instead of writing the resolution file directly).

## Exit Criteria

- A user can fully resolve a bundle via CLI only: init -> pick decisions -> apply -> publish.
