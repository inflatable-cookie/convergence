# 085 Dashboard Recommendations

Status: complete
Updated: 2026-07-25
Roadmap: `g02.023`

## Objective

Make Root answer "what should I do next" — ranked, counted, with the
people who are waiting named.

## Scope of the actual problem

Spec 002 §4.7 asks for ranked recommendations with owner labels. Batch
17.4 deferred it for a good reason: the inbox already ranks and names
runnable commands, and duplicating that on the dashboard needs a ranking
rule that is not just "what the inbox happened to emit first".

So the batch is mostly about finding that rule, and about putting it
somewhere both surfaces read — a dashboard that ranked its own copy
would be a second rule waiting to drift from the first.

## In Scope

- a stated ranking rule, applied in `converge_cli::inbox_actions` so
  every front-end reads one order
- grouped recommendations: kind, count, owners, and where to act
- Root renders them; `Enter` runs the top one

## Out Of Scope

- per-profile reordering (spec §4.6): still parked on a design partner
- a second ranking anywhere

## Acceptance Criteria

- the rule is stated and tested; the Inbox view and the dashboard cannot
  disagree; owners come from real server data

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- **the rule is "what blocks other people, first"**: a superposed bundle
  stops its gate window for everyone, an approval holds up one
  publisher, lane work blocks nobody, and a publication is news. Stated
  on `ActionKind` and applied by sorting inside `inbox_actions`, so the
  Inbox view and the dashboard read one order by construction. The
  existing inbox test asserted the old unranked order and was rewritten
  around the rule rather than adapted to the output
- a group with more than one runnable member offers no command: the
  dashboard reports, it does not pick one of five bundles for you
- **owner labels needed real data.** The first pass read `published_by`
  off the inbox bundle, a field that does not exist — the test passed
  because it invented one. `InboxBundle` now carries `contributors`,
  built from the publications the bundle consumed, which is exactly
  "who is waiting on this". Bounded by `INBOX_CONTRIBUTOR_SCAN`, because
  a coalesced window can hold a hundred publications and a cosmetic
  label should not be the most expensive thing in the response; the cap
  is stated on the wire type so a client knows the list is partial
- `Enter` on Root now runs the top-ranked action. A dashboard that ranks
  work and then makes Enter do something unrelated has not helped.
  Uncaptured local work still wins, because it is the only thing on the
  screen that can be lost
- **the 23.1 finding reappeared and was caught by driving.** The first
  version spelled the command out — `→ converge resolve list` plus 64
  hex characters — which overflowed the row *and* the hint bar, pushing
  the key legend off screen. Rows now name the view, the primary action
  is labelled by kind, and a render test fails if a full id reaches the
  dashboard
- a quiet repo gets no "next" section at all
- the inbox loads at startup and refreshes on events through a
  data-only intent, because the navigating one would have yanked the
  user into the Inbox view every time the dashboard refreshed
- 272 tests green

## Next Task

Batch card 23.5 (superposition detail).
