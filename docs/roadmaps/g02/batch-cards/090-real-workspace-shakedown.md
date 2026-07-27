# 090 Real Workspace Shakedown

Status: complete
Updated: 2026-07-27
Roadmap: `g02.022`

## Objective

Use Convergence for real work, on a real project, and fix what that
finds.

## Scope of the actual problem

`g02.023` established this the hard way. Every batch in it found defects
by driving the real thing that its own tests could not see: a capability
nobody could grant, a token printed twelve characters long, an access
token written to a log file, a hint bar naming the wrong key on six
screens, a wizard drawing over the view behind it, a dashboard
recommendation that did not do what it recommended.

None of those were exotic. They were all *first contact with use*. This
batch is that, at the scale of a whole project rather than one screen.

The distinction from every prior batch: **the operator drives.** This
card's job is to make that possible, capture what comes back, and act on
it. Findings that are cheap and clear get fixed here; findings that
change a subsystem get their own card rather than being smuggled in.

## In Scope

- a real project under Convergence, doing real work over real time
- everything that breaks, recorded — including the things that merely
  annoyed
- fixes for what is cheap and clear
- new cards for what is not

## Out Of Scope

- **releasing.** 22.5 is gated behind this batch, and stays gated until
  the operator says so
- scope creep from findings: a finding that wants a subsystem gets a
  card, not an improvised batch

## Acceptance Criteria

- findings recorded in a log with enough detail to act on later; the
  cheap ones fixed; the expensive ones scheduled; an explicit statement
  of whether the thing is ready to put in front of anyone else

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

34 findings, in
`docs/logs/2026-07/26-010000-batch-22-4-shakedown-findings.md`. A Tauri
todo app was built under Convergence over a working day: 20 snaps, two
identities, two lanes, a resolved superposition, a release consumed cold
by a third workspace, a git mirror, encrypted secrets in use, and a
deployment backed up and restored.

Everything cheap and clear was fixed. Two findings were scheduled
instead: gate-graph administration (33) went to the backlog wanting a
roadmap, and per-snap authorship (29) is noted where the mirror
attributes commits.

### What the shakedown was actually for

Six findings would have cost a real user real work, and no test suite
had seen any of them:

- `snap` outside a workspace would have captured the entire home
  directory (finding 6)
- ignore rules matched only the top level, so a 40-file project
  published as 33 MB — and three copies of the check existed, two of
  which were fixed first (9)
- `doctor --deep`, the tool the operator guide names for proving a
  restore, reported "nothing wrong here" for a repo it could not verify
  (21)
- every aborted publish leaked storage GC could never reclaim (27)
- `sync pull --materialize` silently replaced a colleague's committed
  work with no warning and no way to know a restore would bring it back
  (30)
- `retention set` followed by `gc --execute` wedged a gate permanently,
  with no recovery path through the CLI (34)

The last two arrived in the final stretch, from the last two surfaces
driven. That is the honest reading of the finding rate: it did not fall
off.

### Is it ready to put in front of anyone else?

Yes, with one stated limit and one caveat.

The limit is finding 33. `promote` cannot be reached, because the gate
graph cannot be changed after `repo create`. A repo works, and works
well, as a single gate: publish, resolve, approve, release. Anyone told
otherwise will go looking for a staged-review flow that is implemented
and unreachable. Either 026 lands first, or the limit is documented
where people will read it before they plan around it.

The caveat is that this batch found two permanently-destructive defects
in its last day. Both are fixed and pinned by tests that fail without
the fix — which was worth verifying, because the first version of the
retention test passed either way. But a finding rate that has not
flattened is not evidence of a clean surface.

Nothing found in 34 findings called the design into question. The
failures were in reach, in reporting, and in what the product told
people to type — not in the semantics. Base-aware merge, supersession,
provenance replay, the secret substrate and the capability model all did
what the docs said they would, on real history, first time.

## Next Task

Batch card 22.5 (release) — **only when the operator says the shakedown
is done**. Finding 33 is the open question in front of that: gate
administration before release, or ship single-gate and say so.
