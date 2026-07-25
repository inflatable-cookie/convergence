# 082 TUI Reality Check And Simplification Sweep

Status: complete
Updated: 2026-07-25
Roadmap: `g02.023`

## Objective

Drive the real TUI against a real server, record what is wrong, and take
things out. Nothing is added.

## Scope of the actual problem

The TUI has forty reducer tests and has never been in front of a person.
Reducer tests prove the state machine does what it was told; they say
nothing about whether the hint bar is telling the truth, whether a pane
is mostly empty, or whether four batches of additions left a mode nobody
needs.

Building 23.2's secrets view onto that would be adding a screen to an
interface nobody has driven.

## Method

A pty harness drives the real binary against a real server with a real
workspace: two people, a published bundle, a released channel, a live
superposition, a personal key, and two secrets. Screens are captured by
replaying the terminal output through an emulator, so what is recorded
is what a person would see.

## In Scope

- a findings note covering every screen, recorded in `docs/logs/`
- the subtractions those findings justify
- fixes to anything the driving proves is simply wrong

## Out Of Scope

- new screens, new wizards, new panes: 23.2 onward
- inventing a second key scheme (see the macOS finding): a subtraction
  batch is the wrong place to add a navigation layer

## Acceptance Criteria

- findings recorded from a real session, not from reading the code
- the surface is smaller than it started; every removal is named
- reducer tests still green, and the ones covering removed behaviour are
  removed rather than adapted

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

Nine findings, recorded in full in the batch log. The three that matter:

- **`secret` could not be granted to anyone.** `member add` validated
  against a hand-written capability array that never gained `Secret`
  when g02.019 added it, so the one documented way to grant it refused
  it and only admins — who subsume everything — could touch a secret.
  Two roadmaps of secret work and an adversarial suite never caught it,
  because every suite made its actors admins. The second list is gone:
  `Capability::ALL` lives on the enum and `member add` derives from it
- **the hint bar named the wrong key on six screens.** `primary_action`
  branched on the Local/Remote mode, not the screen, so History, Inbox,
  Bundles, Releases, Lanes and Gates all advertised `Enter: history`
  while Enter did something else. Now per screen, with a test walking
  every view
- **the Local/Remote mode was withholding half of Root from itself.**
  Two dashboards behind a Tab toggle, each using four to six lines of a
  thirty-line pane, each holding what the other one lacked. Removed
  entirely: one Root, `Tab` means completion only, and the rule that
  auto-switched mode when you typed a remote verb went with it

Removed: the `Context` enum and everything hanging off it, the second
Root view, `Tab`'s second meaning, the unreadable per-row `[Enter: …]`
suffix, and three reducer tests covering the deleted mode. Added: two
tests. Net 247 → 246.

Recorded rather than fixed, with reasons: the `Alt` jump layer is inert
on stock macOS terminals (fixing it means adding a key scheme, which is
not what a subtraction batch does — Help now says so); Superpositions
asks for a decision it does not show (23.5); a bundle id is only ever
printed once; the agent trace records screens it cannot reproduce.

## Next Task

Batch card 23.2 (secrets view).
