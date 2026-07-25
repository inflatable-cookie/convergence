# 023 TUI Completion

Status: planned
Owner: repo maintainers
Updated: 2026-07-25

## Context

`g02.017` closed the TUI's structural gaps and recorded what it
deliberately left undone in `docs/rebuild/002` §8: the remaining
wizards, ranked dashboard recommendations, and the list+detail split for
superpositions. Each was deferred with a trigger rather than forgotten.

Two of those triggers have effectively fired. The verb surface has grown
twice since — repos, members, keys, secrets — and every one of those is
console-only, which is exactly the flag-surface obstacle the wizard
deferral named. And secrets now have no view at all, so the one part of
the product that most needs a careful interface has the least.

**The TUI has also never been used.** It has been built, refactored
across four batches, and covered by forty reducer tests, and no human
has sat in front of it. Reducer tests prove the state machine does what
it was told to do; they say nothing about whether the thing is pleasant,
or whether four batches of additions left surfaces nobody needs.

So this roadmap opens by *using* it and taking things out, before adding
anything. Building more screens onto an interface nobody has driven is
how a product accumulates surfaces that each made sense alone.

## Findings Addressed

- the TUI has never been driven by a person, only by unit tests
- four batches of additions (17.1-17.4, plus palette growth in 19.3 and
  20.1) have never been followed by a subtraction
- spec §5 wizards still unbuilt: Bootstrap, Sync, Fetch,
  Release/Promote, Member, Move/rename, Gate-graph edits
- no Secrets view, despite `secret` being in the palette since 20.1
- remote dashboard shows no ranked recommendations with owner labels
  (spec §4.7); the inbox ranks, the dashboard does not
- Superpositions is a flat list; the spec's 65/35 list+detail split
  would show variant content rather than a summary line

## Execution Plan (batch details in cards)

- **23.1 Reality check and simplification sweep**: drive the real TUI,
  including through the agent trace it was built to expose (spec §4.3);
  record what is confusing, redundant, or dead; then *remove*. Nothing
  is added in this batch. Its output is a findings note and a smaller
  surface
- **23.2 Secrets view**: who can read what, value age, stale recipients,
  and the confirm-once paths for rotate and unshare. The audit output
  from 20.2-20.3 is already the right shape; this gives it a screen
- **23.3 Wizard set**: the flag-heavy verbs first — Member, Release and
  Promote, Fetch — reusing the back-one-step and review pattern already
  built. Scope depends on 23.1: a wizard for a screen 23.1 deletes is
  work nobody needed
- **23.4 Dashboard recommendations**: ranked next actions with counts
  and owners, sourced from the same `inbox_actions` the console uses so
  the two cannot disagree
- **23.5 Superposition detail**: the 65/35 split with variant preview,
  and a reducer test suite for the new panes

## Exit Criteria

- somebody has used the TUI for real work and the findings are recorded
- the surface is smaller than it was before 23.2 adds anything
- every verb with more than two flags is reachable without memorising
  them
- secrets have a first-class surface with the safety rails the CLI has
- spec §8's "deferred" list shrinks to items still genuinely waiting

## Next Task

Scheduled after `g02.021`. Opens at batch card 23.1 (reality check and
simplification sweep) — deliberately a subtraction before any addition.
