# Phase 020: Identity And Authentication

Current status:
- Started: persisted users/tokens, bearer middleware resolves Subject, token endpoints.

## Goal

Make identity real and verifiable.

The authority should be able to answer "who is this request" in a durable, auditable way, and the client should provide a first-class UX for managing credentials.

This phase replaces the current dev-only single-token/single-user model with:
- multi-user identities
- per-user access tokens
- proper token verification (hashed-at-rest, revocable, optionally expiring)
- client login/logout and token storage improvements

## Why This Phase Exists

Today the server uses a single `--dev-token` and treats every request as the same `--dev-user`.
That is sufficient for local prototyping but it breaks:
- provenance/audit (publisher/approver attribution)
- lane heads (`heads/me`) semantics
- permissions (team membership) being meaningful

## Scope

In scope:
- Server:
  - persisted user registry
  - persisted access tokens (hashed)
  - bearer auth middleware resolves `Subject` from token
  - endpoints for self-service token management
  - bootstrap mechanism for first admin
  - update permissions/lane membership to reference real users
- Client:
  - `login/logout/whoami`
  - safer token storage (avoid plaintext in `.converge/config.json`)
  - TUI shows current identity

Out of scope:
- External identity providers (OIDC/SAML), SCIM provisioning.
- Hardware-backed device identity, signing keys, or mTLS.
- Fine-grained per-endpoint OAuth scopes beyond basic read/publish/admin.

## Architecture Notes

### Identity primitives

- `UserId` (stable, opaque)
- `UserHandle` (human-facing, unique)
- `AccessToken`:
  - `token_id` (public identifier)
  - `secret` (only shown once)
  - `hash` (stored)
  - `user_id`
  - `created_at`, `last_used_at`, `revoked_at`, optional `expires_at`
  - optional `label` for UX

### Token format

Recommended MVP format:
- token presented by the client is a single opaque string
- server stores `blake3(secret)` (or argon2 in a later hardening pass)
- constant-time compare on hash match

### Subject

`Subject` should carry at least:
- `user_id`
- `handle`
- any server-side role flags (admin)

### Storage

MVP options:
- Continue storing server metadata as JSON on disk (like `repo.json`) but add:
  - `users.json`
  - `tokens.json`

Later:
- move metadata to SQLite/Postgres

## Tasks

### A) Server: user + token store

- [x] Add `User` and `AccessToken` structs.
- [x] Persist users and tokens under the server `data_dir`.
- [x] Implement token hashing at rest (never persist plaintext).
- [x] Implement token revocation + optional expiry checks.

### B) Server: auth middleware

- [x] Replace `--dev-user/--dev-token` fixed identity with:
  - bearer token lookup -> token record -> user -> `Subject`
- [x] Update `GET /whoami` to return `{ user_id, handle }`.
- [x] Update provenance fields to store `user_id` (or store both id+handle).

### C) Server: bootstrap and admin

- [ ] Add a bootstrap mode for first admin creation (one of):
  - CLI flag `--bootstrap-token` that allows `POST /bootstrap` once
  - or generate an on-disk one-time token on first run
- [x] Add admin-only endpoints:
  - create user
  - add/remove repo readers/publishers
  - add/remove lane members

### D) Server: self-service token API

- [x] `POST /tokens` (create token; returns plaintext once)
- [x] `GET /tokens` (list; no plaintext)
- [x] `POST /tokens/:id/revoke`

### E) Client: credential storage + UX

- [x] Stop storing remote token plaintext in `.converge/config.json`.
- [x] Introduce `.converge/state.json` (or OS keychain) to store secrets.
- [x] Add CLI commands:
  - `converge login --url ... --token ... --repo ... [--scope ... --gate ...]`
  - `converge logout`
  - `converge whoami`

### F) TUI: show identity

- [x] TUI header shows `user@server` (from `whoami`) when remote is configured.
- [x] Clear error state when token invalid/expired; show guidance to re-login.

### G) Tests

- [x] Server auth tests:
  - invalid token rejected
  - revoked token rejected
  - token resolves correct user
- [ ] Provenance attribution test:
  - two different users publish/sync and server stores distinct identities

### H) Docs

- [ ] Update `docs/architecture/09-security-identity-and-permissions.md` with concrete token model.
- [ ] Add a short operator doc: how to bootstrap, create users, and mint tokens.

## Exit Criteria

- Server supports multiple users and multiple access tokens.
- Bearer auth resolves a real user identity; provenance and lane heads are attributable.
- Tokens are stored hashed at rest and can be revoked.
- Client provides `login/logout/whoami`, and token is not stored plaintext in `.converge/config.json`.
