# 096 Frame And Navigation

Status: ready
Updated: 2026-07-28
Roadmap: `g02.027`

## Objective

Make it obvious that the TUI can be navigated, and how.

## Scope of the actual problem

The operator, 2026-07-28:

> It's not obvious that there is any navigation structure. It prompts to
> press enter to approve… something, not obvious what, only other option
> is to quit.

That is not an impression. It is literally true, and the cause is a
finding this project already made and then declined to act on. From the
batch 23.1 log:

> `Alt+h`, `Alt+i`, `Alt+b` … are the entire shortcut layer. Terminal.app
> and iTerm send composed characters for Option unless "Use Option as
> Meta key" is enabled, so on the platform most likely to run this, none
> of it fires and nothing says why.
>
> Not fixed here: inventing a second key scheme is an addition, and this
> batch does not add. Typing the verb still reaches every view, so this
> degrades rather than blocks.

Every claim there is correct except the last, and the last is the one
that mattered. "Typing the verb still reaches every view" is only true
for somebody who knows the verbs — which is exactly what a first-time
user does not. For everybody else the entire shortcut layer is dead, and
the only keys the screen offers are Enter, Esc, Tab and q.

So the TUI has had no working navigation for the whole of `g02.023`,
behind a defect that was found, understood, written down accurately, and
deferred on process grounds. The scope rule was applied correctly and
produced the wrong answer: a batch whose stated purpose was a reality
check let a finding through because fixing it counted as "adding".

## The decision

Operator's call, 2026-07-28: **keys navigate, `:` types.**

Bare keypresses always navigate. Typing a command requires an explicit
`:` (or `/`) first, as in `lazygit`, `k9s` and `vim`. The alternative —
bare keys navigating only while the input happens to be empty — was
rejected for making one key do two things depending on state you cannot
see, which is a quieter version of the problem being fixed.

The cost is honest and accepted: typing a verb becomes one keystroke
longer. What it buys is the only property that matters here, that a
person who has never seen this screen can press a key and watch
something happen.

## In Scope

- a navigation mode that is the default, and a command mode you opt into
- single, unmodified keys for every destination, shown on screen rather
  than learned
- numbered destinations where a screen lists things to go to
- the five-band frame from `v0-legacy`: header, body, status, guidance,
  input
- one semantic palette, defined once
- dimming behind modals, which is how a modal reads as modal

## Out Of Scope

- the guidance panel's *content* (27.2) and per-screen layout (27.3)
- removing the command line. It is the argv contract made visible and
  the reason an agent and a person drive the same verbs

## Design Notes

`Alt` must stop being load-bearing. It may remain as an accelerator for
anyone whose terminal sends Meta, but nothing may be reachable *only*
through it, and no hint may name a key that silently does nothing on the
platform the operator uses.

Every key the hint bar offers has to be tested through a pty, because
the failure mode here is precisely a key that exists in the reducer and
never arrives.

## Acceptance Criteria

- from the root screen, with no prior knowledge, every view is reachable
  by a key that is visible on screen
- no shortcut requires a modifier
- pressing `:` and typing a verb still works, and Esc leaves
- the operator can reach bundles, lanes and history without being told how

## Validation

- `effigy validate`
- a pty drive proving each advertised key arrives

## Next Task

Batch card 27.2 (guidance).
