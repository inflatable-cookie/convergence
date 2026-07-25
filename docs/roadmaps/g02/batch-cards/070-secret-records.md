# 070 Secret Records

Status: complete
Updated: 2026-07-25
Roadmap: `g02.019`

## Objective

Storage and transport for encrypted secrets: a record type, a `secret`
capability, version-guarded writes, and endpoints that move ciphertext
without ever looking inside it.

## Scope of the actual problem

19.1 gave people keys and nothing to seal. This batch is the envelope
service: the server has to hold bytes it cannot read, hand them only to
recipients, and refuse a write that would clobber a concurrent one —
the same guarded-batch discipline publish and promote already use.

The subtle part is that a capability alone is not sufficient. `admin`
subsumes every capability (doc 14 §4), so a repo admin holding `secret`
would be able to fetch any ciphertext. That is survivable — they still
cannot decrypt it — but doc 19 §7 says only a *recipient* may fetch, so
the endpoint checks recipient membership on top of the grant.

## In Scope

- `SecretRecord` in both metadata backends, keyed `(repo, owner, name)`
- `Capability::Secret`, and a recipient check that the capability does
  not replace
- `MetaOp::AssertSecretVersion` + `PutSecret`, so a stale write fails
  the batch rather than silently winning
- `PUT/GET/DELETE /api/repos/:repo/secrets/:name`, `GET .../secrets`
- ciphertext armored (ASCII) so a database row is inspectable as an age
  file and obviously not plaintext
- `secret.changed` events carrying name and version, never content

## Out Of Scope

- the CLI verbs (19.3) — this batch ends at the wire
- read auditing (`secret.read`) and consumption surfaces (19.5)
- multi-recipient sharing (`g02.020`): the field is a list from the
  start, but only the owner goes in it here

## Acceptance Criteria

- a recipient can round-trip ciphertext through the server; a
  non-recipient with `secret` cannot fetch it; a stale-version write is
  refused; the server has no code path that parses ciphertext, pinned by
  a test that stores deliberate garbage and gets it back byte-exact

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `SecretRecord` in both backends keyed `(repo, owner, name)`, with
  `PUT/GET/DELETE /api/repos/:repo/secrets/:name` and a listing that
  returns summaries without ciphertext
- **the recipient check is separate from the capability, and that is the
  point.** `admin` subsumes every capability, so a repo admin holding
  `secret` would otherwise be able to pull every envelope in the repo.
  A test registers an admin and asserts the fetch fails
- a non-recipient gets the same 404 as a missing secret. Whether a
  particular secret exists is itself something a non-recipient has no
  claim to, and a distinguishable "403 exists but not for you" would
  leak exactly that
- listing is open to any member on purpose: knowing a secret exists is
  what lets someone ask to be added to it, and the summary carries no
  ciphertext
- writes go through `apply_batch` with `AssertSecretVersion`, so a
  writer working from a stale read gets a 409 instead of silently
  erasing a concurrent rotation. Creating over an existing secret is the
  same mistake with `expected_version` 0, and fails the same way
- ciphertext is a `String` and the server never parses it. The test
  stores deliberate garbage — not an age file at all — and gets it back
  byte-exact, which is the only way to show the absence of a code path
- only the owner may delete: a recipient can read a secret, not destroy
  someone else's copy of it
- refused at the door rather than discovered later: empty ciphertext, an
  empty recipient list (a secret nobody could ever read), and names
  outside a narrow grammar, since names travel in a URL path and land in
  a database key
- 209 tests green

## Next Task

Batch card 19.3 (secret verbs).
