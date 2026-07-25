# 019 Secret Substrate

Status: in progress (19.1 ready)
Owner: repo maintainers
Updated: 2026-07-25

## Context

Convergence stores your remote token in plaintext: `state.json` holds
`remote_tokens` as a bare map on disk, and doc 14 §4 already admits
authentication is slice-grade. Meanwhile a team using Convergence has
nowhere to put the credentials their work depends on.

This roadmap builds the substrate: client-side encrypted secrets where
the server holds nothing it can open. Architecture and threat model:
`docs/architecture/19-secrets-and-key-management.md`. Shared team
secrets follow in `g02.020`, which depends on this.

## Findings Addressed

- remote tokens stored unencrypted in the workspace state file
- no mechanism for a person to keep any credential in Convergence
- identity carries grants and tokens (batch 16.3) but no key material,
  so there is nothing to encrypt *to*

## Execution Plan (batch details in cards)

- **19.1 Key identity**: `converge key init|list|rotate`; X25519 keypair
  via `age`; private key encrypted at rest under a passphrase-derived
  key; public keys registered through the membership surface
- **19.2 Secret records and endpoints**: `SecretRecord` storage in both
  metadata backends, a `secret` capability, version-guarded writes, and
  endpoints that move ciphertext without inspecting it
- **19.3 Secret verbs**: `converge secret set|get|list|rm`, reading and
  writing through the argv contract like every other surface
- **19.4 Token migration and adversarial tests**: the workspace's own
  remote token becomes a locally-encrypted secret; wrong-key refusal,
  tamper detection, and a test proving the server cannot decrypt what it
  stores

## Exit Criteria

- a person can store and retrieve a credential that no other member and
  no operator can read
- the server has no code path that inspects ciphertext, pinned by test
- `state.json` no longer holds a plaintext token
- losing a key loses the secrets, and the interface says so before it
  happens rather than after

## Next Task

Batch card 19.1 (key identity).
