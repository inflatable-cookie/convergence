# 083 Secrets View

Status: complete
Updated: 2026-07-25
Roadmap: `g02.023`

## Objective

Give the part of the product that most needs a careful interface the one
surface it does not have: who can read what, when the credential last
changed, and what has gone stale.

## Scope of the actual problem

`secret` has been in the palette since batch 20.1 and has had no screen.
The data is already the right shape — `secret audit` joins secrets,
members and registered keys and reports readers plus stale recipients —
so this is a rendering job, not a new endpoint.

The interesting question is what the screen may *do*. Batch 20.4 found
the trap that makes this urgent: rotating a secret after someone leaves
re-seals the new value to them, because rotation preserves recipients
and departure does not clear them. The audit flags it. A screen that
shows the flag and offers no way to act on it is a nag.

## In Scope

- `View::Secrets`, loaded through `secret audit` like every other view
  loads through a verb
- readers, owner, value version and value age per secret; stale
  recipients called out
- `u` to unshare every stale recipient the audit flagged, confirm-once
- `r` for rotate
- values never rendered

## Out Of Scope

- sharing with a new subject: that needs a name typed, which is 23.3's
  wizard work
- `secret get` from the screen, at all

## Acceptance Criteria

- the screen answers "who can read this" from real data; no value
  reaches it; the stale-recipient fix is one key and one confirmation;
  reducer and render tests cover both

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- the view loads from `secret audit`, so it cannot show anything a CLI
  user cannot reach, and it leads with **value version and value age** —
  the question an audit actually asks is when the credential changed,
  not when its recipient list did (batch 20.3's distinction)
- `u` unshares *every* recipient the audit flagged, in one argv, because
  leaving one behind is the exact state that caused the complaint
- **driving it found a design error in the batch itself.** `u` re-seals,
  re-sealing opens the private key, and unlocking prompts for a
  passphrase — which `rpassword` writes straight to the tty this program
  holds in raw mode. The prompt drew across the header and then hung,
  waiting for keystrokes the event loop was eating
- the fix is a rule rather than a patch: `needs_private_key(argv)` names
  every verb that must unlock a key, and those are **handed over as a
  command to run in a terminal** instead of half-run. This also closes a
  pre-existing bug from 19.3 — typing `secret get X` into the console
  had the same hang. `CONVERGE_PASSPHRASE` lifts it, because then
  nothing prompts
- values carry a stricter rule on top: they must never enter the input
  buffer even when a passphrase is available, since the buffer is
  echoed, submitted lines land in `command_history`, and `↑` replays
  them. `secret rotate` is handed over unconditionally
- the in-pane hint tracks that state, so it cannot repeat 23.1's finding
  by advertising a confirmation for something that hands over
- **the Last strip only trimmed inside `record_result`**, so anything
  pushing directly grew the vector past the strip's height and its own
  line was the one clipped off the bottom — the hand-over message never
  appeared. One `say()` now pushes and trims
- 23.1's pty harness is replaced by checked-in **render tests**: they
  draw into a ratatui `TestBackend` and assert on the text, so "the hint
  bar names the wrong key" and "History eats its own message" now fail
  in CI instead of waiting for someone to sit in front of it
- 255 tests green

## Next Task

Batch card 23.3 (wizard set).
