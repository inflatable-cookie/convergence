# 023 TUI Completion

Status: in progress (23.1-23.4 complete)
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

- **23.1 Reality check and simplification sweep** (complete, card 082):
  drove the real binaries against a real server through a pty. Nine
  findings, three of them defects: `secret` could not be granted to
  anyone, the hint bar named the wrong key on six screens, and the
  Local/Remote mode kept half of Root hidden from the other half. The
  mode is gone. Net 247 → 246 tests
- **23.2 Secrets view** (complete, card 083): loaded from `secret
  audit`; readers, value age and stale recipients, with `u` unsharing
  every flagged recipient in one command. Driving it found that any verb
  opening the private key cannot run in a raw-mode terminal at all — the
  passphrase prompt draws over the screen and fights the event loop — so
  those are handed over instead, which also closed the same hang for
  `secret get` typed into the console since 19.3. The pty harness is
  replaced by checked-in render tests
- **23.3 Wizard set** (complete, card 084): Member, Release, Promote and
  Fetch, plus `p`/`e` on Bundles rows. Found four defects, three older
  than the batch: the Login wizard put an access token on screen and in
  the agent trace file, `member add --issue-token` truncated a
  shown-once token into uselessness, wizard execution skipped the
  confirm-once rule, and the wizard overlay never cleared the view
  behind it
- **23.4 Dashboard recommendations** (complete, card 085): ranked by
  **what blocks other people, first**, with the sort inside
  `converge_cli::inbox_actions` so every surface reads one order.
  `Enter` on Root runs the top one. Owner labels needed a real field —
  the first pass read one that did not exist — so `InboxBundle` now
  carries bounded `contributors`. Driving it caught 23.1's overflow
  finding reintroduced in a new place
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

Batch card 23.5 (superposition detail).
