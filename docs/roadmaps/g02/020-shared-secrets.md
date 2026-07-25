# 020 Shared Secrets

Status: in progress (20.1-20.3 complete)
Owner: repo maintainers
Updated: 2026-07-25

## Context

`g02.019` gives each person secrets only they can read. The case teams
actually hit is the shared one: a deploy key, a registry token, a CI
credential that several people need. Multi-recipient encryption makes
that possible; membership change makes it hard.

Sequenced after 019 because sharing is the same record with more
recipients — the substrate has to exist first.

## Findings Addressed

- individual secrets do not cover the credential a team shares
- membership is dynamic: people join and leave, and the recipient list
  has to track that without pretending removal is revocation
  (doc 19 §6)

## Execution Plan (batch details in cards)

- **20.1 Multi-recipient secrets** (complete, card 074): `converge
  secret share|unshare`, sealing to every registered key of every
  recipient; owner-aware resolution with `--owner`, fixing a defect
  where two people holding the same secret name silently served whoever
  sorted first
- **20.2 Membership change** (complete, card 075): `converge member
  remove`, refusing to strand a repo without an admin; the removal
  response names every secret still sealed to the departing member;
  `converge secret audit` flags recipients who left or whose key is
  gone. Automatic re-encryption is impossible by design (doc 19 §7), so
  the work is reported to whoever can do it rather than claimed
- **20.3 Rotation workflow** (complete, card 076): `converge secret
  rotate`, plus the fix for a live defect — `set` sealed to the writer's
  own keys, silently unsharing everyone else on every update.
  `value_version` separates a rotation from a re-share so an audit can
  answer "when did this credential last change?"
- **20.4 Adversarial**: a removed member cannot read the next version;
  a stale recipient list cannot silently persist; concurrent share and
  rotate do not lose a recipient

## Exit Criteria

- a team credential is readable by exactly its current recipients
- no interface describes recipient removal as revoking access
- membership changes leave no secret encrypted to a key that should no
  longer receive new versions

## Next Task

Batch card 20.4 (adversarial).
