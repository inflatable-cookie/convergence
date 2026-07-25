# 022 Ship Readiness

Status: planned
Owner: repo maintainers
Updated: 2026-07-25

## Context

Convergence has never been installed by anyone who did not build it.
There is no release artifact, no install path, no upgrade story, and no
operator guidance for backing up a server that now holds credentials
nobody else can decrypt.

That last point is not a nicety. Doc 19 §1 concedes availability
explicitly: the server can delete a secret it cannot read, and
durability is a backup question. Until there is a documented backup and
restore path, that sentence is an unpaid debt.

The gap here is exposure rather than defects. A first real workspace
will find things no test does, and everything in this roadmap exists to
make that meeting possible.

## Findings Addressed

- no release binaries and no install path; the only way in is `cargo
  build` from a clone
- no upgrade story. "Pre-1.0: stores re-init" is fine between us and
  indefensible once someone has real history
- no operator backup/restore guidance, which secrets make load-bearing
- no first-run diagnostic: when something is misconfigured, the user
  gets a verb-level error rather than a picture
- the nightly external-backend lane has never completed a run

## Execution Plan (batch details in cards)

- **22.1 Release and install**: tagged release workflow building
  binaries for macOS and Linux, checksums, and an install path that is
  one command; `converge --version` reporting something traceable to a
  commit
- **22.2 Upgrade and compatibility**: a store-format version with a
  refusal that names the mismatch; a documented policy for what breaks
  when, replacing "re-init" before anyone has history worth keeping
- **22.3 Operator guide**: deploy, back up, restore, and verify a
  restore — including the secrets case, where a lost object store is
  unrecoverable by design and the backup is the only mitigation
- **22.4 First-run diagnosis**: `converge doctor` reporting workspace,
  remote, identity, key, and clock state in one answer, with each
  failure naming its fix
- **22.5 Real workspace**: run Convergence against a genuine project,
  record what broke, and fix what that surfaces

## Exit Criteria

- someone can install, deploy, use, back up, and restore without reading
  the source
- an incompatible store is refused with an explanation rather than
  misread
- the backend lane has run green against live services
- the findings from 22.5 are recorded, and either fixed or scheduled

## Next Task

Blocked behind `g02.021` (or promoted ahead of it if the operator wants
a real user before another subsystem).
