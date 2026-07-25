# Batch 23.2 — Secrets View Complete

Date: 2026-07-25
Roadmap: `g02.023`
Card: `083-secrets-view`

## What Shipped

A Secrets screen, loaded through `secret audit` like every other view
loads through a verb. It shows owner, readers, value version, value age,
and stale recipients — and it leads with the value, because the question
an audit actually asks is when the *credential* changed, not when its
recipient list did.

`u` unshares every recipient the audit flagged, in one command. Leaving
one behind is the exact state that produced the warning, so acting on
the whole list is the fix.

Batch 20.4 found the trap that made this worth building: rotating after
someone leaves re-seals the new value to them, because rotation
preserves recipients and departure does not clear them. The audit has
flagged that since 20.2. A screen that shows a flag and offers no way to
act on it is a nag.

## The Batch Found Its Own Design Error

Driving the finished screen against a real repo — a real departed
member, a real stale recipient — pressing `u` printed `passphrase:`
across the header and hung.

Unsharing re-seals. Re-sealing opens the caller's private key. Unlocking
prompts, and `rpassword` writes the prompt straight to the tty this
program holds in raw mode: it drew over the interface and then waited
for keystrokes the event loop was already eating.

The fix is a rule, not a patch. `needs_private_key(argv)` names every
verb that must unlock a key — `secret get|set|rotate|share|unshare|
write-env`, `key init|rotate`, `run` — and those are handed over as a
command to run in a terminal rather than half-run. `CONVERGE_PASSPHRASE`
lifts the restriction, because then nothing prompts.

That also closed a pre-existing bug from batch 19.3: typing `secret get
X` into the TUI console had exactly the same hang, and had since the
verb shipped. Nobody had typed it.

`secret list` and `secret audit` are deliberately not on the list. They
read metadata the server already holds in the clear — doc 19 §9's stated
trade — which is why the screen can exist at all.

## A Second, Stricter Rule For Values

Even with a passphrase available, a secret value must never enter the
input buffer: it is echoed on screen, submitted lines are pushed into
`command_history`, and `↑` replays them. Typing a credential would
persist it in three places at once.

So `secret rotate` is handed over unconditionally, and the screen says
so in as many words. Recorded in doc 19 §11 as a constraint on *any*
front-end, not just this one.

## Also Fixed

The Last strip only trimmed inside `record_result`. Anything that pushed
a line directly grew the vector past the strip's three-line height, and
its own line was the one clipped off the bottom — so the hand-over
message was written and never seen. One `say()` now pushes and trims,
and every caller uses it.

The in-pane key hint tracks whether a passphrase is available, so it
cannot repeat batch 23.1's finding by advertising a confirmation for
something that hands over.

## The Harness Is Now Checked In

23.1's pty harness was throwaway. It is replaced by render tests that
draw into a ratatui `TestBackend` and assert on the resulting text. Four
of them, covering the new screen and the two 23.1 findings most likely
to regress: the hint bar naming each screen's own key, and History
leaving room for the message beside its id.

Reducer tests prove the state machine does what it was told. These prove
the screen says what it means. The difference is exactly what 23.1 spent
a batch discovering by hand.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 255 passed, 4 skipped
- driven against a real server with a real departed member

## Next Task

Batch card 23.3 (wizard set).
