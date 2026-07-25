# 019 Secret Substrate

Status: in progress (19.1-19.4 complete)
Owner: repo maintainers
Updated: 2026-07-25

## Context

Convergence stores your remote token in plaintext: `state.json` holds
`remote_tokens` as a bare map on disk, and doc 14 §4 already admits
authentication is slice-grade. Meanwhile a team using Convergence has
nowhere to put the credentials their work depends on.

Storage is only half of it. Secrets mostly end up in an environment
variable, and `.env` files are a bad fit for a repository an AI agent
works in. The answer is not a better file format — an agent that can run
`converge secret get` is as dangerous as one that can read `.env` —
but a separate agent identity without the `secret` capability, plus a
delivery path that never writes plaintext into the tree (doc 19 §10).

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

- **19.1 Key identity** (complete, card 069): `converge key
  init|list|rotate`; X25519 keypair via `age`; private key sealed under
  a passphrase and stored `0600` under `CONVERGE_HOME`, not the
  workspace; public keys registered against the token's subject
- **19.2 Secret records and endpoints** (complete, card 070):
  `SecretRecord` in both backends, a `secret` capability *plus* a
  separate recipient check (admin subsumes capabilities, so the grant
  alone would let an admin pull every envelope), version-guarded writes
  through `apply_batch`, and ciphertext the server never parses
- **19.3 Secret verbs** (complete, card 071): `converge secret
  set|get|list|rm`; values enter only through stdin (no `--value` flag,
  pinned by test, because argv lands in shell history and `ps`); each
  secret is sealed to every key the caller holds so a rotation strands
  nothing
- **19.4 Token migration and adversarial tests** (complete, card 072):
  the remote token moves out of the workspace to `CONVERGE_HOME`,
  encrypted under a machine key, migrating on first read (doc 19 §8a
  states what that is and is not worth); wrong-key refusal, tamper
  detection, and a test that reads every byte the server persisted
- **19.5 Consumption** (doc 19 §10): `converge run --secret NAME -- cmd`
  as the default path; `secret get --json` as the seam for injectors
  that already exist; a loud `secret write-env` escape hatch that warns,
  self-ignores, and audits; `secret.read` events; redaction in the TUI
  Last strip, the agent trace, and error messages

## Exit Criteria

- a person can store and retrieve a credential that no other member and
  no operator can read
- the server has no code path that inspects ciphertext, pinned by test
- `state.json` no longer holds a plaintext token
- losing a key loses the secrets, and the interface says so before it
  happens rather than after
- a secret reaches a process without being written to the working tree,
  and every read is on the events feed
- a subject without the `secret` capability cannot reach a credential by
  any surface — the property that makes an agent identity meaningful

## Next Task

Batch card 19.5 (consumption).
