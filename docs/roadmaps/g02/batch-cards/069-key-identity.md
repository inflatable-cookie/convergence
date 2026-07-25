# 069 Key Identity

Status: complete
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

## Outcome

- `converge key init|list|rotate`, with the private half sealed by
  `age`'s scrypt recipient and written `0600`. The on-disk file is an
  age file: a test asserts it does not contain `AGE-SECRET-KEY`, because
  "encrypted at rest" is the kind of claim that quietly stops being true
- **keys live under the user's home, not the workspace** (`CONVERGE_HOME`
  overrides). An identity is a person, not a checkout; per-workspace
  keys would mean a second clone is a second identity that existing
  secrets were never sealed to
- **the subject comes from the token, never the request body.** Letting
  a caller name someone else would let them register a key that future
  secrets get encrypted to — the entire guarantee, given away in one
  field. Pinned by a test that registers under one token and checks the
  recorded subject
- the server parses the recipient before storing it. A malformed key
  would otherwise sit in the table and fail later at encryption time,
  somewhere much harder to diagnose
- every path in the identity module has an explicit-home variant, so
  tests point somewhere other than a developer's real keys without
  mutating a process-wide env var while other tests run
- `key init` prints the no-recovery warning *before* generating
  anything, since afterwards it is only an explanation of what was lost.
  `--yes` skips it for scripts that already told the human
- `key init` works without a remote and says `registered: false` rather
  than failing a local operation that succeeded
- rotation keeps the old key: secrets sealed to it stay readable until
  19.3 can re-encrypt them
- 205 tests green

## Next Task

Batch card 19.2 (secret records and endpoints).
