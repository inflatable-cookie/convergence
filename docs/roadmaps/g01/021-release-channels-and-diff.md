# Phase 021: Release Channels And Diff

## Goal

Close the loop on the core lifecycle by adding first-class releases (named channels that point at bundles) and a minimal `converge diff` CLI.

After this phase, Convergence supports:

`snap -> publish -> bundle -> resolve -> promote -> release -> fetch/restore`

## Scope

In scope:
- Server-side release records and endpoints.
- Release channels (named, mutable pointers with history).
- GC retention roots include releases.
- Client CLI commands to create/list/show releases.
- Ability to fetch/restore a release (materialize the release bundle root manifest).
- Minimal `converge diff` for workspace vs HEAD snap and snap vs snap.

Out of scope:
- Build artifacts, SBOMs, signing, attestations.
- Policy-rich release rules (per-channel/per-gate rules) beyond basic promotability + terminal gate default.
- Exporting releases to Git.

## Tasks

### A) Server: release model

- [x] Add a `Release` record and persist it under repo data.
- [x] Add release provenance fields (`released_by`, `released_by_user_id`, `released_at`).
- [x] Validate release channel id.

### B) Server: release API

- [x] `POST /repos/:repo_id/releases` (create a release).
- [x] `GET /repos/:repo_id/releases` (list all releases).
- [x] `GET /repos/:repo_id/releases/:channel` (get latest release in channel).
- [x] Enforce permissions and promotability; default to terminal gate unless admin.

### C) Server: retention/GC

- [x] Treat releases as retention roots (keep referenced bundles and reachable objects).

### D) Client: remote API bindings

- [x] Add `Release` types and remote client methods (`create_release`, `list_releases`, `get_release`).

### E) CLI: release commands

- [x] `converge release create --channel <name> --bundle-id <id> [--notes ...]`.
- [x] `converge release list`.
- [x] `converge release show --channel <name>`.

### F) Fetch/restore releases

- [x] Add `converge fetch --bundle-id <id>`.
- [x] Add `converge fetch --release <channel>`.
- [x] Support `--restore [--into <dir>]` for bundle/release fetch.

### J) TUI: release support

- [x] Add remote command `release --channel <name> --bundle-id <id>`.
- [x] Add bundles-mode shortcut `release <channel>` for selected bundle.

### G) Minimal diff

- [x] `converge diff` (workspace vs HEAD snap).
- [x] `converge diff --from <snap_id> --to <snap_id>`.

### H) Tests

- [x] Server release API test (create/list/show; retention root behavior smoke test).
- [x] CLI e2e test (create release; fetch/restore release).

### I) Docs

- [x] Update `docs/architecture/03-operations-and-semantics.md` release section with concrete API/CLI shape.

## Exit Criteria

- A release channel can be created from a promotable bundle.
- Releases are listable and fetchable.
- A release can be restored into a directory deterministically.
- Releases prevent GC from deleting required objects.
- `converge diff` exists for basic local inspection.
