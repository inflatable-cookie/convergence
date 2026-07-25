# 074 Multi-Recipient Secrets

Status: complete
Updated: 2026-07-25
Roadmap: `g02.020`

## Objective

Let a secret be readable by several people: `converge secret
share|unshare`, sealing to every current key of every recipient.

## Scope of the actual problem

The record already carries a recipient list and the server already
checks membership, so the encryption side is mostly wiring. The
interesting part is addressing.

Secrets are keyed `(repo, owner, name)`, but `find_secret` resolves by
**name alone** — it takes the first match from a listing ordered by
owner. Two people with a `db-password` today means whoever sorts first
wins, silently. Personal secrets made that unlikely to be noticed;
sharing makes it a correctness problem, because "the secret called X"
stops being unambiguous the moment more than one person can hold one.

So this batch fixes resolution as well: prefer the caller's own secret,
fall back to the single one they can read, and refuse rather than guess
when several match.

## In Scope

- owner-aware resolution, with `--owner` / `?owner=` to disambiguate
- `converge secret share NAME --with SUBJECT` — decrypt, re-seal to the
  union of recipients' keys, write a new version
- `converge secret unshare NAME --with SUBJECT` — re-seal without them,
  and say plainly that this is not revocation (doc 19 §6)
- sealing to *every* registered key of each recipient, so a teammate who
  has rotated is not locked out

## Out Of Scope

- automatic re-encryption when someone joins or leaves a repo (20.2)
- `secret rotate` as a first-class verb (20.3)

## Acceptance Criteria

- two people can read one secret; a third cannot; unsharing removes
  future access and says what it does not do; two owners can hold the
  same name without either being served the wrong one; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- **the resolution defect was real and is fixed.** `find_secret`
  matched on name alone and took the first record from a listing ordered
  by owner, so two people holding a `db-password` meant whoever sorted
  first won — silently, and with no way to tell. Resolution now prefers
  the caller's own, falls back to the single one they can read, and
  refuses with the owners named when several match. `--owner` /
  `?owner=` disambiguates
- `secret share NAME --with SUBJECT` decrypts, re-seals to the union of
  recipients, and writes a new version through the same guard as any
  other write. There is no server-side shortcut and doc 19 §7 says there
  must not be one: sharing is an encryption-time decision
- sealing covers **every registered key of every recipient**, so a
  teammate who has rotated is not locked out by a share that only saw
  their old key. Sharing with someone who has no key at all fails with
  their name and the command they need to run
- `secret unshare` refuses the word "revoke" in its own output and says
  the credential must be rotated at its source. A test asserts both the
  presence of the rotation advice and the absence of "revoke", because
  this is the one place the interface could quietly lie
- unsharing closes future versions only, which the test demonstrates by
  writing a new version afterwards and checking the removed recipient
  cannot read it
- 226 tests green

## Next Task

Batch card 20.2 (membership change).
