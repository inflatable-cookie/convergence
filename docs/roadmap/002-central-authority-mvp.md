# Phase 002: Central Authority MVP (Publish + Fetch)

## Goal

Introduce a minimal central authority (GitHub-like) that can:
- authenticate users
- register a repo with a basic gate graph/scope
- accept `publish` submissions from clients
- serve the stored objects back to clients (`fetch`)

This phase proves the large-org-first premise: identity + access control + authoritative namespaces, without yet implementing full convergence (bundling) semantics.

## Scope

In scope:
- Server:
  - identity/authn (minimal workable: local dev tokens)
  - authz (repo-level + lane-level publish permissions)
  - repo registry
  - gate graph registry (static config initially)
  - scope registry
  - publication intake (metadata + content)
  - object distribution (blobs/manifests/snaps) to clients
  - audit/provenance records for publish actions
- Client:
  - `converge login` (or equivalent token setup)
  - `converge publish` (upload snap + metadata to server)
  - `converge fetch` (download missing objects)
  - `converge status` that can show:
    - current scope
    - latest known published items relevant to the user’s lane

Explicitly out of scope:
- `converge bundle` (gate coalescing), `promote`, release channel creation.
- Complex policy DSL/external CI integration (policy is informational only here).
- TUI.
- Background capture.

## Architecture Notes

Principles:
- The server is authoritative for identity, permissions, and naming.
- Snaps remain immutable objects; publishing does not rewrite history.
- Publish should not block on merge/conflict. (Conflicts become superpositions later.)

Suggested minimal server implementation choices (can change):
- Rust HTTP API (e.g. axum)
- Postgres or SQLite for metadata (SQLite acceptable for dev; Postgres target)
- Content store:
  - on-disk CAS initially
  - later: S3-compatible blob store

## Object Model (Phase 2 subset)

Server must store and serve:
- blobs
- manifests
- snaps
- publications (references a snap + target gate + scope)

Minimum provenance:
- publisher identity
- timestamp
- client/workspace identifiers (best-effort)

## Tasks

### A) Server skeleton

- [x] Create Rust server crate/binary.
- [x] Basic config (ports, data dirs, DB connection).
- [x] Health endpoint.

### B) Identity and auth

- [x] Implement a minimal identity model:
  - [x] users
  - [x] access tokens
- [x] Implement request authentication via bearer token.

### C) Authorization

- [x] Implement repo-level permissions:
  - [x] who can read
  - [x] who can publish
- [x] Implement lane membership as a first pass (even if lanes are "one default lane" initially).

### D) Repo + gate graph + scope registry

- [ ] Implement endpoints to:
  - [x] create a repo
  - [x] define a minimal gate graph (can be hard-coded "dev-intake" only initially)
  - [x] create scopes

### E) Object upload API

- [x] Endpoints to upload:
  - [x] blobs by id
  - [x] manifests by id
  - [x] snaps by id
- [x] Server validates:
  - [x] IDs match hashes
  - [ ] manifests reference existing blobs/manifests (or allow staged upload ordering)

### F) Publish intake API

- [x] Endpoint: create publication referencing a snap.
- [x] Validate:
  - [x] user has permission to publish to target repo/scope/gate
  - [x] snap exists (or upload as part of publish)
- [x] Store publication metadata + provenance.

### G) Object fetch API

- [x] Endpoints to fetch:
  - [x] blobs
  - [x] manifests
  - [x] snaps
- [x] Support a "missing objects" workflow:
  - [x] client sends list of ids, server returns which are missing
  - [x] client uploads only missing

### H) Client changes

- [x] Add remote configuration to workspace metadata.
- [x] Implement `converge publish`:
  - [x] ensure local snap exists
  - [x] upload missing objects
  - [x] create publication
- [x] Implement `converge fetch`:
  - [x] fetch referenced objects for publications/bundles the user can see (Phase 2: publications only)
  - [x] store in local cache

### I) Minimal UX

- [ ] `converge status` should show:
  - [ ] configured repo/remote
  - [ ] current scope
  - [ ] most recent publications visible to the user (by lane)
- [ ] Add `--json` variants for publish/fetch/status.

### J) Tests

- [ ] Server API contract tests (happy path + authz failures).
- [ ] Upload integrity tests (hash mismatch rejected).
- [ ] End-to-end test: init -> snap -> publish -> fresh workspace fetch -> restore.

## Exit Criteria

- A developer can:
  - create a snap locally
  - publish it to the server under a scope and gate
  - from a second machine/workspace, authenticate, fetch objects, and restore that snap
- Server enforces at least basic permissions.
- Provenance exists for publish events.

## Follow-on Phases

- Phase 003: Gates + `converge bundle` + promotability + `promote`.
- Phase 004: TUI (inbox + superpositions + promotion workflow).
- Phase 005: Rich policy execution (CI integration) + release channels and artifacts.
