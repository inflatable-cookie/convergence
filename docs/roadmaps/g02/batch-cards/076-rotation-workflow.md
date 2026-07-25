# 076 Rotation Workflow

Status: complete
Updated: 2026-07-25
Roadmap: `g02.020`

## Objective

Make rotating a credential a first-class move, and make an audit able to
tell when a secret's *value* last changed as opposed to when its
recipient list did.

## Scope of the actual problem

Writing this card found a live defect. `secret set` always seals to the
caller's own keys, so updating a shared secret **silently unshares
everyone else**. No message, no failure — the next teammate to read it
just cannot, long after the change that caused it.

Batch 20.1's test did not catch it: that test unshared before updating,
so the update sealing to one key looked correct. A test can only catch
what it distinguishes.

The second half is the audit question. `version` counts every write, so
sharing bumps it exactly like a new value does. An operator asking "has
this credential been rotated since the contractor left?" cannot answer
from that. The server cannot tell the difference either — a re-seal
produces different ciphertext whatever the plaintext was — so the client
has to say which it did, and the server records the assertion.

## In Scope

- `secret set` preserves an existing secret's recipients, resolved to
  those subjects' current keys
- `converge secret rotate NAME` — new value, same recipients, recorded
  as a value change
- `value_version` and `value_updated_at` on the record;
  `value_changed` on the write request
- `secret audit` shows when each secret's value last changed

## Out Of Scope

- proving a client's `value_changed` claim: the server cannot, and a
  client that lies only misleads its own team's audit. Documented rather
  than defended against

## Acceptance Criteria

- updating a shared secret keeps every recipient; rotating bumps the
  value version and sharing does not; audit distinguishes the two; all
  suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- **the defect is fixed and now has a test that distinguishes.** `set`
  preserves an existing secret's recipients, and resolves them through
  their subjects' *current* keys — sealing to the stored key ids would
  have swapped one silent failure for another by locking out anyone who
  had rotated
- `secret rotate` is `set` with a name that says what happened. Worth a
  verb because "I rotated this credential" is a claim someone needs to
  make and an audit needs to show
- `value_version` and `value_updated_at` separate a rotation from a
  re-share. `version` counts every write, so sharing bumped it exactly
  like a new value did, and "has this been rotated since the contractor
  left?" was unanswerable
- the server cannot verify the `value_changed` claim: a re-seal produces
  different ciphertext whatever the plaintext was. So the client asserts
  and the server records the assertion; a client that lies misleads only
  its own team's audit, which is stated rather than defended against
- `secret audit` reports when each value last changed, next to who can
  read it — the two facts an operator needs in the same place after
  somebody leaves
- 230 tests green

## Next Task

Batch card 20.4 (adversarial).
