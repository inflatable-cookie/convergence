# 022 Ship Readiness

Status: in progress (22.1-22.4 complete; 22.5 gated)
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

## Ordering: Local Use Before Release

**Nothing in this roadmap publishes anything.** The operator's stated
position (2026-07-25) is that they want to use Convergence thoroughly on
their own machine before it goes anywhere, so the batches are ordered to
serve that and the release batch is explicitly gated.

That ordering is also the better engineering. `g02.023` established the
pattern the hard way: every batch found defects by driving the real
thing that its own tests could not see. A release cut before a real
workspace has been used is a release of untested exposure — the point of
22.4 is to find those defects while the only person affected is the one
who can fix them.

So: diagnosis and install first (they make local use possible), then the
format guarantees that matter *before* real history exists, then the
operator story, then the shakedown, and only then the release — which
does not run until the operator says so.

## Findings Addressed

- no release binaries and no install path; the only way in is `cargo
  build` from a clone
- no upgrade story. "Pre-1.0: stores re-init" is fine between us and
  indefensible once someone has real history
- no operator backup/restore guidance, which secrets make load-bearing
- no first-run diagnostic: when something is misconfigured, the user
  gets a verb-level error rather than a picture
- the nightly external-backend lane has never completed a run
- no way to stand up a throwaway local deployment to exercise the thing
  end to end without hand-assembling one each time

## Execution Plan (batch details in cards)

- **22.1 Local diagnosis and install** (complete, card 087): `converge doctor`
  reporting workspace, remote, identity, key, server reachability and
  clock skew in one answer, each failure naming its fix; a local install
  path; `--version` traceable to a commit. No packaging, no publishing
- **22.2 Store format and upgrade refusal** (complete, card 088): a version stamp
  on the workspace and server stores, with a refusal that names the
  mismatch and what to do. This lands *before* 22.4, because the point
  of 22.4 is to accumulate real history and "re-init" stops being an
  acceptable answer the moment that history exists
- **22.3 Operator guide** (complete, card 089): deploy, back up, restore, and
  **verify a restore** — including the secrets case, where a lost object
  store is unrecoverable by design and the backup is the only
  mitigation. Written against a real local deployment, not from memory
- **22.4 Real workspace shakedown** (complete, card 090): 34 findings
  over a working day of real use — a Tauri todo app, two identities, a
  resolved superposition, a release consumed cold, a backup restored.
  Everything cheap and clear fixed; gate-graph administration went to
  the backlog. Six of the findings would have cost a real user real
  work, and no test suite had seen any of them. The verdict on the card:
  ready for others with one stated limit — `promote` is unreachable
  until the gate graph can be changed (finding 33)
- **22.5 Release** (card 091, **gated**): tagged release workflow,
  binaries for macOS and Linux, checksums, one-command install. Does not
  start until the operator says the shakedown is done. Touches
  `.github/workflows/`, which needs an explicit instruction anyway

## Exit Criteria

- the operator can stand up, use, break, back up and restore a local
  deployment without reading the source
- an incompatible store is refused with an explanation rather than
  misread
- `converge doctor` answers "why is this not working" without a verb by
  verb hunt
- the findings from 22.4 are recorded, and either fixed or scheduled
- the backend lane has run green against live services
- 22.5 remains unstarted until explicitly released

## Next Task

`g02.026` (gate administration), and then batch card 22.5.

The single-gate question is decided (operator, 2026-07-27): gate
administration lands first. The release pipeline is built and proven as
far as a laptop allows, and waits — a release mechanism proven early
costs nothing, and a release rushed to meet a mechanism costs the
thing itself.
