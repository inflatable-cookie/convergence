# 069 Key Identity

Status: ready
Updated: 2026-07-25
Roadmap: `g02.019`

## Objective

Give every subject key material: an X25519 keypair whose private half
never leaves the client and whose public half is registered with the
server, so there is something to encrypt secrets *to*.

## Scope of the actual problem

Batch 16.3 gave identity grants and tokens — everything needed to
authorize, nothing that can hold a secret. A token proves who you are to
the server; it cannot make the server unable to read your data. That
takes a keypair the server never sees the private half of.

## In Scope

- `age` (the `rage` crate) as the primitive; no hand-assembled AEAD
- `converge key init` — generate a keypair, encrypt the private key at
  rest under a passphrase-derived key, register the public key
- `converge key list` — the caller's registered keys, and whose they are
- `converge key rotate` — register a new key; re-encryption of existing
  secrets lands in 19.3 once secrets exist to re-encrypt
- server: public keys on the membership surface, `POST/GET
  /api/repos/:repo/keys`, both metadata backends
- the no-recovery warning at `key init`, confirmed once — a security
  property nobody warned you about is a trap

## Out Of Scope

- secrets themselves (19.2, 19.3): this batch ends with keys registered
  and nothing yet encrypted
- hardware-backed key storage (doc 19 §9, deferred with trigger)
- shared recipients (`g02.020`)

## Acceptance Criteria

- a subject can generate a keypair, register it, and list it; the
  private key on disk is unreadable without the passphrase; the server
  stores only public keys; a wrong passphrase fails loudly; all suites
  green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Next Task

Batch card 19.2 (secret records and endpoints).
