# Batch 23.1 — TUI Reality Check And Simplification Sweep

Date: 2026-07-25
Roadmap: `g02.023`
Card: `082-tui-reality-check`

## Method

A pty harness drove the real binaries against a real server: two people
(`root` and `bob`), a published bundle, a channel release, a live
superposition on `docs/plan.md`, a personal key, and two secrets. Screens
were captured by replaying terminal output through an emulator, so what
is recorded below is what a person would see, not what the code says.

The session also ran the TUI under `--agent-trace`, which worked as
specified.

## Findings

Ordered by what they cost someone, not by where they live.

### 1. `secret` could not be granted to anyone

`member add` validated capabilities against a hand-written array that
never gained `Capability::Secret` when g02.019 added it. So the one
documented way to grant it refused it — `unknown capability secret` —
and the only subjects who could touch a secret were admins, who subsume
everything.

This is the finding that justifies the batch. Two roadmaps of secret
work, a full adversarial suite, and the grant path was broken the whole
time, because no test ever granted `secret` to a non-admin: the suites
made their actors admins.

Fixed by deleting the second list. `Capability::ALL` lives on the enum,
`member add` derives from it, and a test now grants every capability the
server defines.

### 2. The hint bar named the wrong key on six screens

`primary_action()` branched on the Local/Remote *mode* and the
Superpositions view, and nothing else. So the bottom bar read
`Enter: history` on History (where Enter restores, with a confirm), and
on Inbox, Bundles, Releases, Lanes and Gates, where Enter runs the
selected row's action — `handle_rows_key` runs first and the bar never
knew.

A hint bar that names the wrong key is worse than no hint bar, because
it is believed. It is now computed per screen, with a test that walks
every view.

### 3. The Local/Remote mode was withholding half the story from itself

Root had two variants behind a Tab toggle. The local one showed head,
pending changes and automatic captures. The remote one showed the
target, last published snap and last seen bundle. Each used four to six
lines of a thirty-line pane, and a person orienting themselves wants all
of it at once.

The mode's other jobs were a header colour, a prompt label, and a rule
that auto-switched mode when you typed a remote verb — a rule that
exists only because the mode does.

**Removed.** One Root shows everything. `Tab` is completion and nothing
else, instead of meaning two things depending on whether the input was
empty. Spec 002 §7 listed "dual home dashboards" as a wart and proposed
*labelling* the mode; deleting it is the smaller interface.

### 4. History hid the column people read

Rows led with the full 64-character snap id, which pushed the message
off the right edge at any sane width. Every other list view already used
`short_id`. Now History does too, and timestamps are trimmed to seconds.

### 5. The `Alt` jump layer does nothing on stock macOS terminals

`Alt+h`, `Alt+i`, `Alt+b` … are the entire shortcut layer. Terminal.app
and iTerm send composed characters for Option unless "Use Option as Meta
key" is enabled, so on the platform most likely to run this, none of it
fires and nothing says why. Typing the verb still reaches every view, so
this degrades rather than blocks.

Not fixed here: inventing a second key scheme is an addition, and this
batch does not add. Help now states the requirement, and the limit is
recorded in spec 002 §8.

### 6. Superpositions asks for a decision it will not show you

The view lists `docs/plan.md  [2 variants]  undecided` and offers `1-9
pick`. There is no way to see what either variant contains. You are
choosing between two file contents, blind.

Spec §6's 65/35 list+detail split was deferred as polish. Driving it
shows it is not polish: it is the difference between a decision and a
guess. Batch 23.5 addresses it, and the deferral note now says why.

### 7. Inbox rows spelled out a 64-character id nobody could read

Each row appended `[Enter: resolve list <full bundle id>]`, always cut
off at the right edge. The hint bar already names what Enter does.
Removed.

### 8. A bundle id is only ever printed once

Not a TUI finding, recorded because the session hit it. `publish` prints
the bundle id; `inbox` lists a bundle only when it needs *your* action,
so a bundle that is immediately ready (a gate with zero required
approvals) never appears there for anyone. `events` has it, and `events`
is documented as "hints; reconcile via inbox".

The release path works and is the intended route for a teammate
(`fetch --release stable --checkout` checked out cleanly). But an admin
who published and closed their terminal has no first-class way back to
the id. Left for a later batch rather than fixed mid-sweep.

### 9. The agent trace records what, not where

`screen_view` lists `selectable_items` but not which one is selected,
and arrow keys produce no `user_action` entry. A trace therefore
describes a screen it cannot reproduce. Recorded, not fixed.

## What Was Removed

- the `Context` enum, its toggle, its header colour, its prompt label
- the second Root view
- the auto-cross-the-boundary rule for remote verbs
- `Tab`'s second meaning
- the per-row `[Enter: …]` suffix in the Inbox
- three reducer tests that only covered the removed mode

Two tests were added: per-screen primary actions, and every capability
being grantable. Net 247 → 246.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 246 passed, 4 skipped
- re-driven through the pty harness after the changes

## Next Task

Batch card 23.2 (secrets view).
