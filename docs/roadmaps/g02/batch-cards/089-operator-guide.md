# 089 Operator Guide

Status: complete
Updated: 2026-07-25
Roadmap: `g02.022`

## Objective

Deploy, back up, restore, and **verify the restore** — written against a
real deployment rather than from memory.

## Scope of the actual problem

Doc 19 §1 concedes availability in one sentence: the server holds
secrets it cannot read, so it cannot regenerate them, and a lost object
store loses them permanently. That sentence is an unpaid debt until
there is a backup procedure somebody has actually run.

Guide 004 §6 currently says "stop the server, tar the data directory".
That is probably right and definitely untested. The gap between those
two words is the batch.

The part usually skipped is verification. A backup nobody has restored
is a hypothesis, and the failure mode is discovering that at exactly the
moment it matters. So the procedure ends with a restore into a *second*
deployment and a check that the secrets still open — which is the only
test that exercises the thing that cannot be regenerated.

## In Scope

- deploy: data directory layout, what lives where, what the flags mean
- back up: consistent snapshot, including what "stop it first" is
  actually protecting against
- restore: into a fresh directory, and into a running deployment's place
- **verify**: a restored deployment serves objects, and a secret sealed
  before the backup still decrypts after it
- the external-backend case (Postgres, S3), where "the data directory"
  is not the whole story
- what is *not* recoverable, stated plainly

## Out Of Scope

- automated backup tooling: the procedure first, and only then a
  question about whether it should be a verb
- high availability, replication, failover

## Acceptance Criteria

- every step has been run against a real deployment; the verification
  step catches a deliberately corrupted backup; the unrecoverable cases
  are named

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

Every step was run against a real deployment — build it, back it up,
destroy it, restore it, prove the restore — and the guide was written
from what happened rather than from what should happen.

- **`converge doctor --deep`**, because driving the restore found the
  gap: a deployment whose entire `objects/` directory was missing passed
  every ordinary check and reported "nothing wrong here". The control
  plane was answering, and nothing doctor asked touched the object
  store. `--deep` asks the server whether it still holds the root
  manifest of its own `stable` release — one round trip, no transfer,
  and precisely the question that fails
- **a `fetch` is not a restore test.** A `fetch --release stable --into`
  reported success against that same gutted deployment, because the
  workspace had fetched before and was served out of its own local
  store. Correct behaviour, useless as verification. The guide now says
  to check from a clean workspace, and the automated test uses one
- **two backups, not one.** The server's data directory holds ciphertext;
  `~/.converge` holds the keys that open it. Neither is recoverable from
  the other, and losing either is total in its own way. The guide states
  both in a table rather than burying it
- **stop the server first, and here is why**: SQLite runs in
  rollback-journal mode, not WAL, so a transaction in flight leaves a
  `meta.sqlite-journal` beside the database and a tar that catches one
  without the other restores torn
- the whole round trip is an automated test: publish, release, seal a
  secret, copy the directory, serve the copy, and assert the secret
  still decrypts, provenance still replays, and the tree still
  materializes. Plus the mistake case — database without objects —
  asserting plain `doctor` passes it and `--deep` does not
- what is *not* recoverable is stated: a secret whose key is lost, an
  unpublished snap, a token's plaintext

## Next Task

Batch card 22.4 (real workspace shakedown).
