# 075 Membership Change

Status: complete
Updated: 2026-07-25
Roadmap: `g02.020`

## Objective

Make a team that changes shape survivable: remove a member, and make the
consequences for every shared secret visible to the people who can act
on them.

## Scope of the actual problem

You can add a teammate (batch 16.3) and never remove one. That alone is
a gap.

The larger one is what removal *means* for secrets. The obvious feature
— "removing a member re-encrypts the secrets they had" — cannot exist:
the server holds no key that opens anything (doc 19 §7), so only a
person who can already read a secret can re-seal it. Pretending
otherwise would be the worst outcome, because an operator would believe
access was withdrawn when it was not.

So this batch does the honest version: removal happens, and it *reports*
which secrets still seal to the departing member, to whom, and what the
owner must do. The work stays with the person who can do it, and nothing
claims to have done it for them.

## In Scope

- `converge member remove SUBJECT` — drop every grant in the repo,
  admin only, refusing to remove the last admin
- removal output naming the secrets still sealed to that subject
- `converge secret audit` — for secrets you can read: who the recipients
  are, and which of them are no longer members or no longer hold the key
- the rotation reminder, everywhere it applies (doc 19 §6)

## Out Of Scope

- automatic re-encryption on membership change (doc 19 §9: impossible by
  design, named there so its absence reads as a decision)
- `secret rotate` as a verb (20.3)

## Acceptance Criteria

- a removed member loses repo access immediately; secrets still sealed
  to them are listed by name and owner; `secret audit` flags stale
  recipients; nothing describes any of it as revoking access to values
  already read; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge member remove SUBJECT` drops every grant in the repo, keeps
  the user record (secrets are still sealed to their keys, and erasing
  the subject would make those unattributable), refuses to remove the
  last admin, and refuses to remove the caller
- **the removal response names the secrets still sealed to them.** The
  feature an operator expects — "removal re-encrypts what they had" —
  cannot exist, because the server holds no key that opens anything.
  Pretending otherwise would be the worst outcome: an operator would
  believe access was withdrawn when it was not. So removal reports, and
  the report tells the owner exactly which two commands to run
- `converge secret audit` shows readers per secret and flags stale
  recipients two ways: a subject who is no longer a member, and a key id
  nobody has registered any more (rotated away, leaving a dead entry)
- output across removal and audit avoids the word "revoke" and says
  "rotate at the source" instead. A test asserts the absence, because
  this is where an interface most easily tells a comforting lie
- 228 tests green

## Next Task

Batch card 20.3 (rotation workflow).
