# 072 Token Migration And Adversarial

Status: complete
Updated: 2026-07-25
Roadmap: `g02.019`

## Objective

Take Convergence's own credential off the floor — `state.json` holds the
remote token in plaintext inside the workspace — and prove the
substrate's claims by attacking them.

## Scope of the actual problem

The token is a bearer credential to the server, sitting in the
repository tree in cleartext. Anything that reads the repo reads it: a
backup, a stray `cat`, and above all an AI agent exploring the
workspace, which is the case doc 19 §10a exists for.

The honest fix is narrower than "encrypt it". A token encrypted to the
personal key would prompt for a passphrase on every remote command,
which nobody would tolerate — they would go back to a plaintext file
somewhere else. So this batch does the two things that help without
that cost: move the credential **out of the workspace**, and encrypt it
at rest under a machine-local key. That defends against discovery, and
not against a determined same-uid attacker; the doc says so rather than
implying more.

## In Scope

- tokens move to `CONVERGE_HOME`, encrypted under a machine key
  (`0600`), keyed by a hash of the remote so the filename leaks nothing
- one-time migration on read: an existing plaintext token is moved and
  erased from `state.json`
- adversarial tests: wrong key refused, tampered ciphertext refused
  rather than yielding altered plaintext, and the server unable to
  decrypt what it stores

## Out Of Scope

- OS keychain or hardware-backed storage (doc 19 §9, deferred)
- passphrase-protecting the token: the cost outweighs the gain, and
  the reasoning belongs in the doc rather than in a TODO

## Acceptance Criteria

- no plaintext token in the workspace after a `login` or an upgrade;
  remote commands keep working with no prompt; tampering is detected;
  all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- tokens moved to `CONVERGE_HOME`, encrypted under a machine key, in
  files named by a hash of the remote so a directory listing does not
  enumerate which servers a machine talks to. Doc 19 §8a records both
  what that is worth and what it is not
- migration happens on read, not on some upgrade command nobody runs:
  the first remote operation after upgrading moves the token and erases
  the plaintext. A test puts a workspace back into the old shape by hand
  and checks both halves
- the token file is binary age rather than armored — nothing reads it by
  eye, so armor would only add bytes. Secrets stay armored because a
  database row *is* read by eye
- **tamper detection is the test worth having.** An envelope that
  decrypted to *something* after modification would be worse than one
  that refuses, because the caller would act on it. The test flips a
  byte in the armored body and asserts a hard failure
- "the server cannot read your secrets" is now a test rather than a
  claim: it reads every byte the server persisted and asserts the
  plaintext is absent, no private key is present, and the stored
  envelope does not open with an unrelated key
- 219 tests green

## Next Task

Batch card 19.5 (consumption).
