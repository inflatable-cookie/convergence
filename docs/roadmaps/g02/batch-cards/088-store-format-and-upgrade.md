# 088 Store Format And Upgrade Refusal

Status: complete
Updated: 2026-07-25
Roadmap: `g02.022`

## Objective

Make an incompatible store refuse with an explanation, instead of being
misread.

## Scope of the actual problem

The current answer to "what happens when the format changes" is "pre-1.0,
re-init". That is fine between people who can rebuild from source and
indefensible the moment somebody has history they care about.

This lands *before* the shakedown batch on purpose. 22.4 exists to
accumulate real local history; the moment that exists, "re-init" stops
being an acceptable answer, and adding the guard afterwards means the
first store that needed it did not have it.

The failure being prevented is not a crash. A crash would be fine. It is
a **newer binary silently misreading an older store** — a field that
gained a meaning, an id whose domain tag changed (batch 18.3 changed one:
`converge-snap-v3` to `v4`), an enum that gained a variant an old reader
skips. Those corrupt quietly.

## In Scope

- a format version written into the workspace store and the server store
- a refusal on mismatch that names the version found, the version
  expected, and what to do about it
- the refusal is on *open*, not on first bad read
- a documented policy for what changes require what: what is additive,
  what is a bump, what will never migrate
- forward refusal too: an old binary must not open a newer store

## Out Of Scope

- migrations. Refusing correctly is the batch; migrating between formats
  is a decision per change, not a mechanism to build in advance
- versioning the wire protocol, which already has `WIRE_VERSION`

## Acceptance Criteria

- an old store against a new binary is refused by name, and the reverse;
  a store written today is readable by today's binary; the policy is
  written where someone will find it

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- a stamp file per store: `.converge/format` and `<data-dir>/format`.
  Its own file, not `config.json`'s existing `version` field, which
  nothing ever read and which **could not have worked**: config.json is
  parsed by serde, so a shape change fails to parse before anything
  looks at the version
- **absent means 1, permanently**, and nothing rewrites it, so opening a
  store stays a pure read. Load-bearing: batch 22.1's `doctor` opens a
  workspace and is tested to change nothing
- both directions refused, on *open*, so the message can say "Nothing
  has been read or written" and mean it. A stamp of the wrong *kind*
  gets its own message, since a workspace passed as a data directory is
  a different mistake
- **`--force` was a hole, found by driving it.** Every verb refused a
  format-99 workspace; `init --force` then reset it to format 1 and
  destroyed it. Worse, batch 22.1's `doctor` was recommending exactly
  that command. Both fixed: `--force` will not re-initialise a store
  this build cannot read, and discarding one means removing the
  directory by hand — an unmistakable act rather than a casual flag
- doc 16 §3 records what requires a bump: would a binary at the other
  version *misread* this, not "did the bytes change"
- 291 tests green

## Next Task

Batch card 22.3 (operator guide).
