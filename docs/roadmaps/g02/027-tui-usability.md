# 027 TUI Usability

Status: ready
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

- **27.1 Frame and palette** (card 096): restore the five-band frame,
  define the semantic palette in one place so no screen invents its own,
  dim behind modals
- **27.2 Guidance** (card 097): a permanent panel answering "what can I
  do from here", with help text restored to the suggestion data — the
  single biggest thing the rebuild dropped
- **27.3 Screens that look like their subject** (card 098): the root as
  sections rather than a paragraph; list views that show state, not just
  ids
- **27.4 Somebody else drives it** (card 099): the operator, on a repo
  they have not been walked through, with findings recorded the way
  batch 22.4 recorded them

## Exit Criteria

- a person who has not read the source can publish, resolve and promote
  from the TUI without asking what a screen means
- every screen answers "where am I, what is here, what can I do"
- colour carries meaning consistently and nothing depends on colour alone
- the operator's verdict on 27.4 is better than the one that opened this

## Next Task

Batch card 27.1 (frame and palette).
