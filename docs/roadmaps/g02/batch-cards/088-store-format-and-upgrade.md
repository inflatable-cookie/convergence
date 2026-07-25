# 088 Store Format And Upgrade Refusal

Status: ready
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

## Next Task

Batch card 22.3 (operator guide).
