# 19 Secrets And Key Management

Status: active
Updated: 2026-07-25
Roadmap: `g02.019` (individual secrets), `g02.020` (shared secrets)

Per-person and per-team credentials stored in Convergence, encrypted on
the client, with the server holding nothing it can open. This doc owns
the threat model, the object model, and the key lifecycle. Doc 14 owns
authorization; this doc never weakens it.

## 1. Threat model

**Protected against:**

- another repo member reading a secret they are not a recipient of,
  including a repo admin
- the server operator reading secrets from the database, the object
  store, a backup, or a memory dump of the server process
- an attacker with the full server dataset
- tampering: modified ciphertext fails to decrypt rather than yielding
  altered plaintext

**Explicitly not protected against:**

- **availability.** The server can delete a secret it cannot read.
  Confidentiality is the claim; durability is a backup question.
- **a compromised client.** A secret decrypted on your machine is
  plaintext on your machine. Convergence protects storage and transport,
  not your laptop.
- **metadata.** The server sees the repo, the owner, the secret's
  *name*, its size, and when it changed. Encrypting names is possible
  and deferred (§9) — it costs the ability to list without decrypting.
- **a recipient you already shared with.** Someone who has read a shared
  secret has it. Removing them from the recipient list stops future
  reads and nothing else (§6).

**No recovery.** Losing your private key loses every secret encrypted to
it. There is no escrow, no admin override, and no reset — those would
each mean someone other than the owner could read, which is the property
this whole design exists to provide. The consequence is stated at key
creation and in every guide, because a security property nobody warned
you about is a trap rather than a feature.

## 2. Why secrets are not files

The obvious design — put an encrypted file in a snap — is wrong here,
and the reasons are structural rather than stylistic:

- **Dedup leaks.** The store is content-addressed. Deterministic
  ciphertext would make identical secrets share an object id, so the
  server could see that two people hold the same credential without
  decrypting anything.
- **Merge is meaningless.** The fold (doc 17 §2) reasons about content
  it can read. Two encrypted versions of a secret are always "divergent"
  and would superpose forever, offering variants nobody can compare.
- **Publish is broadcast.** Anything in a snap reaches every bundle,
  every `read` grant, and `git export`. A secret on that path is a
  secret leaked to the whole repo by construction.

So: **secrets are a sibling of the object store, not a tree inside it.**
They have their own record type, their own endpoints, their own
lifecycle, and they never enter a manifest, a bundle, a window, or a git
stream. The determinism, verify, and merge stories are untouched because
they never see a secret.

## 3. Object model

```
SecretRecord {
  repo_id, scope_pattern,      // where it lives (doc 14 §4 scope grammar)
  name,                        // opaque to the server, unique per owner
  owner,                       // subject who created it
  recipients: [KeyId],         // who can decrypt; [owner] for a personal secret
  ciphertext: bytes,           // age armored, encrypted to every recipient
  version: u64,                // increments on every write
  updated_at, updated_by
}
```

One record per `(repo, owner, name)`. The ciphertext is opaque: the
server stores and returns bytes and has no code path that inspects them.

Not content-addressed, deliberately. Secrets are mutable in place (a
rotated credential replaces the old one), they must not dedup, and they
have no reason to be reachable from a manifest. Storing them by id in
the object store would put them under GC's reachability rules, where
"unreferenced" would mean "deleted".

## 4. Identity and keys

**Primitive: `age` (the `rage` crate).** X25519 recipients, ChaCha20-
Poly1305 payloads, multi-recipient support in the format itself, and
passphrase (scrypt) encryption for keys at rest. Chosen over
hand-assembled AEAD because the multi-recipient envelope — the thing
shared secrets need — is exactly what age is for, and because the format
is widely deployed and independently reviewed. The Rust crate carries a
`[BETA]` label from its author; the format does not, and that
distinction is worth stating rather than hiding.

**Key lifecycle:**

- `converge key init` generates an X25519 keypair. The private key is
  written to the workspace store encrypted under a passphrase-derived
  key; the public key is registered with the server as part of the
  caller's identity.
- Registration rides the membership surface from batch 16.3: a member
  has a subject, grants, tokens, and now zero or more public keys.
- Multiple keys per subject are allowed, because that is what makes
  rotation and a second machine possible without sharing a private key.
- `converge key rotate` registers a new key and re-encrypts every secret
  the caller can read to the new key. Re-encryption is a client
  operation by necessity — the server cannot do it.

**The server never holds a private key.** It stores public keys and
ciphertext. There is no configuration that changes this; an operator who
wants to read secrets has to compromise a client.

## 5. Individual and shared secrets

The same record shape covers both; only `recipients` differs.

- **Individual:** `recipients = [your key]`. Nobody else can decrypt,
  including admins.
- **Shared:** `recipients = [every current member's key]` for a named
  group. Encryption is multi-recipient, so the plaintext exists once and
  is sealed to N public keys.

Sharing is therefore an *encryption-time* decision, not an access-control
decision. The server's authz still gates who may fetch the ciphertext
(doc 14 §4), but that is defence in depth: the cryptography is what makes
the guarantee, and the grant check is what stops someone hoarding
ciphertext to attack later.

## 6. Revocation is rotation

The honest part, stated once so no interface implies otherwise.

Removing someone from a shared secret's recipient list means they cannot
decrypt *future* versions. It does not un-read what they already read.
If a departing member had access to a credential, the credential is
compromised and must be **rotated at its source** — a new AWS key, a new
API token — and the new value re-encrypted to the remaining recipients.

Convergence therefore:

- refuses to describe recipient removal as "revoking access", in output
  or docs
- prompts, on removal, that the underlying credential should be rotated
- records `version` and `updated_by` so an audit can tell when a secret
  last actually changed, as opposed to when its recipient list did

## 7. Server obligations

The server must:

- enforce that only a recipient may fetch a secret's ciphertext, using
  the existing grant machinery (a `secret` capability, doc 14 §4)
- store ciphertext byte-exact and return it unmodified
- version every write and refuse a write against a stale version, the
  same guarded-batch pattern publish and promote already use (doc 14 §3)
- keep secrets out of events, the inbox, GC's object walk, bundles, and
  provenance replay

The server must not:

- attempt to parse, validate, or re-encrypt ciphertext
- hold any key material beyond registered public keys
- offer any endpoint whose success would imply it can read

## 8. Interaction with existing subsystems

| Subsystem | Interaction |
| --- | --- |
| Snaps, bundles, merge | none — secrets never enter a manifest |
| Git export | none — nothing to export, by construction |
| GC (doc 14 §5) | none — secrets are not objects; they have their own retention |
| Events feed | a `secret.changed` event carrying name and version, never content |
| `verify` / determinism | untouched — provenance replay never reads a secret |
| Local token storage | **first customer**: `remote_tokens` in `state.json` is plaintext on disk today, and becomes a locally-encrypted secret |

## 9. Deferred, with triggers

- **Encrypted secret names.** The server currently sees names. Trigger:
  a deployment where the *existence* of a credential is sensitive.
  Cost: listing requires decrypting every entry.
- **Hardware-backed keys (OS keychain, YubiKey, Secure Enclave).**
  Trigger: a user asking to keep the private key off disk. The
  passphrase path stays as the portable default.
- **Server-side re-encryption on membership change.** Impossible by
  design; the client must do it. Named here only so the absence reads as
  a decision rather than an omission.
- **Secret injection into process environments** (`converge run -- cmd`).
  Trigger: real use of secrets in automation. Deliberately separate: the
  moment secrets enter a subprocess environment, the leak surface is
  process listings and crash dumps, and that deserves its own design.

## Next Task

Implement `g02.019` batch 19.1: key identity, registration through the
membership surface, and the passphrase-protected private key at rest.
