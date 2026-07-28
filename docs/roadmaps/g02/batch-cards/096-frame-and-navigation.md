# 096 Frame And Navigation

Status: complete
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

## Outcome

Bare keys navigate; `:` opens the console. `handle_key` is now two
functions — navigation, and `handle_command_key` for everything that
edits text — so a keypress is never ambiguous about which world it is
in. Per-view keys run first, which keeps a screen's own verbs winning
over the global jumps on that screen (`e` releases a bundle on the
Bundles screen and jumps to Releases everywhere else). `Alt` survives as
an accelerator for free: crossterm reports Alt+h as `h` with a modifier
the match ignores, so nothing requires it and nothing breaks with it.

The footer is the navigation surface: every destination with its bare
key, yellow key against gray label, the primary action first in bold.
It fits a 100-column terminal, which meant shortening the promote CTA —
the row it summarises already names the target gate.

Proven the way the card demanded: a pty pressed every advertised key
and asserted the destination screen arrived. Two keys initially
reported as dead (`i`, `b`) turned out to be the harness racing the
async inbox load — worth recording, because the difference between "the
key does nothing" and "the result is asynchronous" is invisible in a
single frame and is exactly what a person concludes wrongly.

One deliberate behaviour change beyond the split: submitting a command
leaves the console, and recalling history means opening it again.
Typing stays two keystrokes from anywhere; the reducer tests type
through a helper that presses `:` first, and wizards are unaffected
because they capture keys before the mode split.

Deferred to 27.2/27.3, not dropped: the five-band frame with the
guidance panel, and the semantic palette applied beyond the footer.
This card fixed the blocker — navigation now exists and is visible.

## Next Task

Batch card 27.2 (guidance).
