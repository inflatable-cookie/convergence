# 089 Operator Guide

Status: ready
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

## Next Task

Batch card 22.4 (real workspace shakedown).
