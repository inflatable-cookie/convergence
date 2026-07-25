# 077 Shared Secret Adversarial

Status: complete
Updated: 2026-07-25
Roadmap: `g02.020`

## Objective

Attack the sharing model, and close the trap the previous batches
created between them.

## Scope of the actual problem

20.3 made `set` and `rotate` preserve recipients, which is right. 20.2
made removing a member leave their key on every secret they were shared
into, which is also right — the server cannot re-seal.

Put together, those two correct decisions make a trap: **rotating a
credential after someone leaves re-seals the new value to them.** They
cannot fetch it while their grants are gone, so nothing breaks today.
But re-adding them later — or any path that restores `read` and
`secret` — hands them everything rotated in the meantime, and the person
doing the rotation had no reason to suspect it.

Neither batch is wrong on its own. The gap is that nothing says so at
the moment it matters.

## In Scope

- `set` and `rotate` warn when the preserved recipient list includes
  subjects who are no longer members, naming them and the fix
- adversarial tests: a removed member cannot read a later version; a
  stale recipient cannot persist unnoticed; concurrent share and rotate
  cannot lose a recipient

## Out Of Scope

- refusing to rotate with stale recipients: an operator mid-incident
  needs the new value stored, and a hard refusal would send them to a
  worse workaround

## Acceptance Criteria

- rotating with a departed recipient warns and names them; concurrent
  writes conflict rather than losing a recipient; a removed member
  cannot read anything written after their removal; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- **the trap is closed by saying so, not by refusing.** `set` and
  `rotate` warn on stderr when the preserved recipient list includes
  people who have left, naming them and the `unshare` that fixes it.
  Stderr keeps `--json` parseable; a refusal would have sent someone
  mid-incident to a worse workaround
- the warning stops once the fix is applied, which the test checks —
  a warning that never clears is one people learn to ignore
- a removed member cannot read anything written after their removal:
  their grants are gone, so the fetch fails before decryption is even
  relevant
- a stale recipient survives rotation and keeps showing in `secret
  audit`. That is correct and worth pinning: rotating does not quietly
  clear the flag, because the key really is still on the record until
  someone unshares
- concurrent share and rotate resolve through the version guard —
  exactly one lands, the loser is told to retry, and re-running it
  leaves the recipient reading the same value the owner sees
- doc 19 §6 records the interaction between two individually correct
  decisions, which is where this class of gap always lives
- 233 tests green

## Next Task

Roadmap `g02.020` is complete, and with it the secret substrate program
`g02.019`-`g02.020`.
