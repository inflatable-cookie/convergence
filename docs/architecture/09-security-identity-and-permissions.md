# Security, Identity, and Permissions

This document defines the security model for the development authority and client.

## Identity

The server is authoritative for identity.

Identity primitives:
- `user_id`: stable, opaque identifier
- `handle`: human-facing identifier (lowercase `a-z0-9-`)
- `access token`: bearer token secret presented by the client

The server stores identity state on disk (under `converge-server --data-dir`):
- `users.json`
- `tokens.json`

The client stores remote credentials locally in the workspace:
- `.converge/config.json`: non-secret remote config (url/repo/scope/gate)
- `.converge/state.json`: secrets (remote access tokens)

Note: `.converge/` is intentionally gitignored.

Minimum:
- user identities
- service identities (CI runners, gate bots)

Today:
- user identities exist; service identities can be modeled as normal users.

## Authorization model

Permissions are scoped by:
- repo
- lane
- scope
- gate

Current enforcement (dev server):
- Repo read/publish: stored as both handles and user ids on the repo record.
- Lane membership: stored as both handles and user ids on the lane record.
- Admin-only endpoints: user management and membership management.

Core actions:
- `snap` (local; no server permission)
- `publish`
- `converge`
- `promote`
- `release`

The dev server currently focuses on repo and lane permissions. Gate/scope permissions are represented in the model but not yet enforced as separate ACLs.

## Audit and provenance

All server-side state transitions must be attributable:
- publish
- converge
- promote
- release

The server records both handle and user id where applicable (for durability even if handles change in the future):
- publications: `publisher`, `publisher_user_id`
- bundles: `created_by`, `created_by_user_id`, `approval_user_ids`
- promotions: `promoted_by`, `promoted_by_user_id`
- lanes: head updates are attributed by the authenticated `Subject`.

On startup, the server performs best-effort backfills of `*_user_id` fields for older on-disk records.

## Access tokens

Token format (MVP):
- The client presents a single opaque bearer token secret.
- The server stores `blake3(secret)` in `tokens.json` as `token_hash`.
- Revocation/expiry are enforced by the auth middleware.

The client should treat tokens as secrets:
- store them only in `.converge/state.json` (or an OS keychain in a later phase)
- never commit them
- rotate/revoke when exposed

## Bootstrapping

Two dev flows exist:

1) Development auto-bootstrap (default)
- If the identity store is empty and no bootstrap token is configured, the server creates a single admin user from `--dev-user` and `--dev-token`.
- This is convenient for tests and local prototyping.

2) One-time bootstrap endpoint (recommended for shared dev servers)
- Start the server with `--bootstrap-token <secret>` and an empty `--data-dir`.
- Call `POST /bootstrap` once to create the first admin and mint an admin token.
- Restart the server without `--bootstrap-token` once bootstrapped.

## Secret handling

Because snaps can contain secrets:
- implement secret scanning on publish (and optionally on snap creation locally)
- provide redaction and key rotation guidance

## Trust boundaries

- A publication is not automatically trusted.
- Gates decide what inputs are allowed and what checks are required.
