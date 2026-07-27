# 092 Graph Model And Impact Analysis

Status: ready
Updated: 2026-07-27
Roadmap: `g02.026`

## Objective

Decide, without a server in the loop, whether a proposed gate graph is
legal and what changing to it would touch.

## Scope of the actual problem

Gate graph validation currently happens nowhere, because the graph is
written once by `repo create` from a literal the code controls. The
moment a person can supply one, every shape they can type has to have an
answer: a gate whose upstream does not exist, two gates pointing at each
other, a graph with no entry, a strategy nobody implements.

Getting that wrong is not a bad error message. `promote` walks upstreams
and `publish` resolves an entry gate, so a cyclic or entryless graph is
a server that hangs or refuses everything, on data that is already
committed.

The second half is impact. Gates are part of the addressing of live
state — partitions are keyed by gate, bundles belong to one,
publications sit in a gate's open window. Removing a gate strands all of
it, which is exactly the shape of finding 34: a dangling reference with
no way back through the CLI. Something has to be able to say what a
change would touch *before* it touches it, and that something should be
answerable by a pure function over the graph plus a count of what lives
in each gate.

## In Scope

- validation as a pure function over `GateGraph`: unknown upstreams,
  cycles, no entry gate, unknown strategy, duplicate gate ids, a
  `may_release` gate that nothing can reach
- an impact type describing a change: gates added, removed, re-parented,
  and for each removed or re-parented gate, what lives there
- the storage query behind it — bundles, publications and partition
  state per gate — as one call rather than three round trips
- refusal messages that name the gate and the reason, in the shape the
  rest of the product uses

## Out Of Scope

- the HTTP route (26.2) and the CLI (26.3)
- deciding *policy* on what is refusable: this batch reports impact,
  26.2 decides what to do about it

## Design Notes

Validation is a pure function on purpose. It is the piece most likely to
be wrong in an interesting way, and a pure function is the piece that
can be tested exhaustively — including the seeded-property style 18.3
used, where a named seed reproduces exactly.

A cycle check is not optional politeness. `promote` walks upstreams to
decide whether a promotion is legal, so a cycle is an infinite walk in a
request handler.

## Acceptance Criteria

- every illegal graph shape has a named refusal and a test
- impact for a change reports the affected gates and what lives in each
- no server needed to test any of it

## Validation

- `effigy validate`

## Next Task

Batch card 26.2 (server write path).
