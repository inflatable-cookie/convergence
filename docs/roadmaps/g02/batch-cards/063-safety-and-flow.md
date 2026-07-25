# 063 Safety And Flow

Status: complete
Updated: 2026-07-25
Roadmap: `g02.017`

## Objective

Audit P2.10, P2.11, P3.13, P3.15: only `restore` confirmed, the
resolution view had no live validation or jump keys, the Last strip
dumped truncated raw JSON, and the publish wizard hardcoded
`--lane default`, making the personal-lane default unreachable.

## Scope of the actual problem

`approve` and `promote` are one keystroke from an inbox row and are
visible to the whole team the instant they land; `gc --execute` deletes
objects for good. Meanwhile the resolution view — the product's flagship
screen — could not say how many paths were still undecided without the
user counting rows, and the strip meant to explain what just happened
cut JSON mid-token at 120 characters.

## In Scope

- one `confirmation_prompt(argv)` rule, applied to typed commands and to
  inbox rows alike
- `ResolutionState::validation()`: live missing/invalid counts
- `Alt+n` next missing, `Alt+f` next invalid (UX spec §5)
- structured Last strip rendering
- publish wizard lane field: blank means the personal lane

## Out Of Scope

- confirmation for `publish`: it is the primary action on the remote
  screen and is corrected by publishing again, so a prompt there would
  train people to dismiss prompts
- server-side validation on every keystroke: the authoritative check
  already runs inside `resolve apply`

## Outcome

- the confirmation rule is "hard to walk back **for someone else**",
  not "destructive": approve, promote, release, restore, unsnap, and
  `gc --execute` ask; snap, fetch, show, publish, and a dry-run `gc` do
  not. One function, so a verb cannot confirm in the console and skip
  the prompt from a list row
- live validation is pure. `missing` and `invalid` are answerable from
  the variant lists already on screen, so they update per keystroke; a
  validation that needed the store could not be live. `invalid` is a
  decision pointing past its variant list, which happens when a path is
  re-listed after a new publish
- `Alt+n`/`Alt+f` wrap, so the jump keys never dead-end at the bottom
  of the list
- the Last strip renders `key value` pairs over scalar fields, drops
  nulls and empty strings, shortens long ids to twelve characters, and
  puts any `next` field last as `→ converge …`. Verbs already return a
  small set of meaningful keys (batches 16.1-16.2), so there was nothing
  to invent
- the publish wizard's lane field is optional and blank by default;
  omitting `--lane` is what makes the server resolve the caller's
  personal lane. The old `default` literal was a lane id, and a wrong
  one
- 178 tests green, including six new reducer tests

## Next Task

Batch card 17.4 (reducer suite and spec reconciliation) — done,
card 064.
