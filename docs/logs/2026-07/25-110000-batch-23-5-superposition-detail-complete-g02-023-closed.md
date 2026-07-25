# Batch 23.5 — Superposition Detail Complete, g02.023 Closed

Date: 2026-07-25
Roadmap: `g02.023`
Card: `086-superposition-detail`

## What Shipped

`converge resolve list <ref> --preview` returns each variant's key plus a
bounded look at its content, and the Superpositions view renders it as
the 65/35 list+detail split spec §6 specified.

The key is untouched, so the decisions file keeps the shape `resolve
apply` already expects. The preview is additive, not a second format.

Bounded twice: `PREVIEW_BYTES` caps what is read — a chunked file reads
only its first chunk, so the bound holds on the store as well as on the
output — and `PREVIEW_LINES` caps what is shown.

## Refusing To Guess Is The Feature

A binary variant gets no text. It gets `binary, 12 bytes`.

Driving it is what forced the size in. The first version said just
`(binary)` for both variants of `logo.png`, which is not a choice — it
is two identical labels above a prompt asking you to pick one. The size
is usually the thing that tells a 4.1 MB render from a 4.3 MB one.

A tombstone reads `deleted in this variant`, because a deletion is a real
option somebody has to be able to select. A variant whose blob is not
local yet says so rather than erroring, since a lazily fetched bundle
makes that ordinary.

Two screens of replacement characters would have been worse than any of
these.

## The Dashboard's Own Recommendation Did Not Work

Batch 23.4 made `Enter` on Root run the top-ranked action. For a
superposed bundle that argv is `resolve list <id>` — and it ran as a raw
command, printing "2 superposed path(s)" into the Last strip instead of
opening the view it was recommending.

The correct mapping had existed since batch 16.1, private to the inbox
row's key handler: `resolve list <ref>` is the console form, because an
inbox row has to be a command a person can paste, and inside the TUI the
same intent opens the view. Batch 23.4 added a second dispatch site and
did not know about it.

Now shared as `action_for_argv`, used everywhere an inbox argv is
dispatched.

## The Deferral Had Mis-Scoped Itself

Batch 17.4 deferred this with the trigger "variants carrying more than a
source and a size". That framing made it a nice-to-have.

Batch 23.1 drove the flat list and found the screen was asking someone to
choose between two file contents and showing neither — then landing the
choice as a snap. That is a decision-correctness problem. The spec now
records the correction rather than the original trigger.

## g02.023 Closed

All five exit criteria met.

The pattern worth keeping: **every batch in this roadmap found defects by
driving the real binary that its own tests could not see.** A capability
nobody could grant. A token printed twelve characters long. An access
token in a log file. A hint bar naming the wrong key on six screens. A
wizard overlay compositing over the view behind it. A dashboard
recommendation that did not do what it recommended.

Reducer tests prove the state machine does what it was told. They say
nothing about what a person sees. The render tests introduced in batch
23.2 close that gap going forward — they draw into a `TestBackend` and
assert on the text, so this class of defect fails in CI.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 276 passed, 4 skipped
- driven against a real server with a text conflict and a binary conflict

## Next Task

`g02.022` (ship readiness), the remaining scheduled roadmap.
