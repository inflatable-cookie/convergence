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

## Findings Addressed

- spec §5 wizards still unbuilt: Bootstrap, Sync, Fetch,
  Release/Promote, Member, Move/rename, Gate-graph edits
- no Secrets view, despite `secret` being in the palette since 20.1
- remote dashboard shows no ranked recommendations with owner labels
  (spec §4.7); the inbox ranks, the dashboard does not
- Superpositions is a flat list; the spec's 65/35 list+detail split
  would show variant content rather than a summary line

## Execution Plan (batch details in cards)

- **23.1 Secrets view**: who can read what, value age, stale recipients,
  and the confirm-once paths for rotate and unshare. The audit output
  from 20.2-20.3 is already the right shape; this gives it a screen
- **23.2 Wizard set**: the flag-heavy verbs first — Member, Release and
  Promote, Fetch — reusing the back-one-step and review pattern already
  built
- **23.3 Dashboard recommendations**: ranked next actions with counts
  and owners, sourced from the same `inbox_actions` the console uses so
  the two cannot disagree
- **23.4 Superposition detail**: the 65/35 split with variant preview,
  and a reducer test suite for the new panes

## Exit Criteria

- every verb with more than two flags is reachable without memorising
  them
- secrets have a first-class surface with the safety rails the CLI has
- spec §8's "deferred" list shrinks to items still genuinely waiting

## Next Task

Blocked behind `g02.021` and `g02.022`; the smallest of the three, and
reasonable to interleave.
