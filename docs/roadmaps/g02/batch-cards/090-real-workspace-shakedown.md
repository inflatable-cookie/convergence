# 090 Real Workspace Shakedown

Status: ready — operator-driven
Updated: 2026-07-25
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

## Next Task

Batch card 22.5 (release) — **only when the operator says the shakedown
is done**.
