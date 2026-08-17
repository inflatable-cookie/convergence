# 027 TUI Usability

Status: in progress (closing)
Owner: repo maintainers
Updated: 2026-07-28

## Context

The operator, on first real use of the rebuilt TUI, 2026-07-28:

> This TUI is horrible and confusing. The legacy one wasn't perfect, but
> it was much more intuitive than this. It at least showed sections for
> the different features and data you needed to look at, had a bit of
> colour and guided you through the process. The current one is almost
> completely incomprehensible.

That is the verdict that matters, and it should not be argued with. It
also should not be mistaken for taste: all three complaints are specific
regressions against `v0-legacy`, and all three are measurable.

## The first thing to fix

Before any of what follows: **there is no working navigation at all.**
The `Alt` layer is the whole shortcut set and stock macOS terminals send
composed characters for Option, so every jump key silently does nothing.
Batch 23.1 found this, wrote it down accurately, and deferred it on the
grounds that typing the verb still worked — which is true only for
somebody who already knows the verbs. Card 096 has the detail.

## What was actually lost

**Guidance.** The legacy shell rendered a permanent suggestions panel —
nine lines, one row per command, the verb in yellow and *its help text*
beside it, navigable with the selection highlighted. The rebuild kept a
`Vec<String>` of bare verb names, reachable only by pressing Tab. The
data that told you what a verb was for is gone, and so is the surface
that showed it without being asked.

**Colour.** The legacy used colour semantically: cyan for a command,
red for an error, yellow for a verb, white for a value, gray for
chrome. The rebuild makes 24 colour calls and 14 of them are
`DarkGray`. Measured on the root screen: 679 cells at default colour,
90 gray, and 12 cells — 1.5% — carrying any hue at all.

**Structure.** The legacy frame was header / body / status band /
suggestions / input, with modals that dimmed everything behind them. The
rebuild is header / one bordered box / a "Last" strip / an input line.
One box means every screen looks the same and none of them says what
kind of thing you are looking at.

## Why the rebuild lost them

Worth stating, because it is the same lesson this project keeps
relearning and the fix has to account for it.

`g02.017` and `g02.023` built and refined this TUI across eight batches
and seventy reducer tests, and batch 23.1 explicitly opened with "drive
the real thing before adding anything". It did — and found nine real
defects. But every one was a *correctness* defect: a wrong key in a hint
bar, a wizard drawing over the view behind it, a recommendation that did
not do what it recommended.

Reducer tests answer "did the state change correctly". Render tests
answer "does this string appear". Neither can answer "is this
comprehensible", and the pty drives were run by the person who had built
it and knew what every screen meant. The first genuinely new pair of eyes
was the operator's, and the verdict was immediate.

## In Scope

- a guidance surface: what can I do here, what does it do, without asking
- a semantic palette applied consistently, and legible on light and dark
  terminals
- screens that look like the thing they show, rather than one box
- the legacy's modal dimming, which is how a modal reads as modal
- a usability pass driven by somebody who did not build it

## Out Of Scope

- new features. Every verb already exists; this is about reaching them
- rewriting the reducer. `g02.023`'s state machine is sound and its
  tests are worth keeping — this is the layer above it

## Execution Plan (batch details in cards)

- **27.1 Frame and navigation** (card 096): the headline. The `Alt` jump
  layer is the *entire* shortcut set and does nothing on stock macOS
  terminals — a defect batch 23.1 found, recorded correctly and declined
  to fix because "this batch does not add". Keys now navigate and `:`
  types (operator's call). Plus the five-band frame, one semantic
  palette, and dimming behind modals
- **27.2 Guidance** (card 097): a permanent panel answering "what can I
  do from here", with help text restored to the suggestion data — the
  single biggest thing the rebuild dropped
- **27.3 Screens that look like their subject** (card 098): the root as
  sections rather than a paragraph; list views that show state, not just
  ids. *Root done early*, driven by the operator's screenshot: Your work
  / Server / What needs doing panels, a numbered selectable to-do list
  whose highlighted row states what Enter runs, plain words in place of
  field names, and the event tally summarised by kind
- **27.4 Somebody else drives it** (card 099): the operator, on a repo
  they have not been walked through, with findings recorded the way
  batch 22.4 recorded them
- **27.5 Decisions on screen** (card 100): the acts that replace the
  working tree — `restore`, `sync pull --materialize`,
  `fetch --checkout` — were unreachable from the TUI because their guard
  was a paragraph of prose. The guard now answers in structure, so the
  CLI prints it and the TUI draws it with a key per option, and
  `--snap-first` gives both surfaces a way through that costs nothing.
  Opened by the operator: *"this is the point of the TUI — to make these
  complex actions accessible"*

## Exit Criteria

- a person who has not read the source can publish, resolve and promote
  from the TUI without asking what a screen means
- nothing a person can reach from the TUI is guarded by advice they must
  leave the TUI to follow
- every screen answers "where am I, what is here, what can I do"
- colour carries meaning consistently and nothing depends on colour alone
- the operator's verdict on 27.4 is better than the one that opened this

## Next Task

Operator cold-drive verdict on usability. Cards 096, 097, 100 are complete.
If exit criteria are met, close `g02.027` and refresh front doors. List-view
row polish deferred to the drive itself (card 097 notes).
