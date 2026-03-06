# Phase 022: Hardening And Polish

## Goal

Stabilize the now end-to-end Convergence workflow by tightening UX, closing parity gaps, and expanding test coverage so we can enter a focused “tweaking + regression” phase with confidence.

## Scope

In scope:
- CLI/TUI UX polish (help text, hints, consistent guidance).
- Expand automated tests around releases, fetch/restore, and diff.
- Improve status surfaces (CLI + TUI dashboard) so users can see “what’s the current release?”.
- Small correctness fixes discovered during hardening.

Out of scope:
- New major primitives (artifacts, signatures, OIDC, Git export).
- Large refactors (storage backend migrations, major protocol redesign).

## Tasks

### A) UX consistency

- [x] Remove remaining references to legacy `remote set --token ...` and standardize on `login` guidance.
- [x] Update TUI command help/usage strings for new fetch/release capabilities.

### B) Status surfaces

- [x] CLI `converge status` shows releases (latest per channel).
- [x] TUI remote dashboard shows release summary.

### C) Fetch parity

- [x] TUI `fetch` supports `--bundle-id` and `--release` (and optional restore into dir).

### D) Testing

- [x] Add focused tests for `converge diff` (workspace vs HEAD; snap vs snap).
- [x] Add release permission edge-case tests (non-terminal release requires admin).

## Exit Criteria

- Release and fetch workflows have consistent guidance across CLI and TUI.
- Releases are visible via `status` and the TUI remote dashboard.
- Added tests cover the core workflow and common error cases.
