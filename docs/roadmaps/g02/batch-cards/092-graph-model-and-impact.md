# 092 Graph Model And Impact Analysis

Status: complete
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

## Outcome

`converge_model::gates`: `validate` returning every fault rather than
the first, and `impact_of` comparing two graphs against caller-supplied
occupancy. Both pure, both tested without a server — fifteen tests.

Decisions worth keeping:

- **every fault, not the first.** One round trip per problem is the
  experience `converge doctor` was built to end, and a graph editor
  should not reintroduce it
- **cycle detection is not politeness.** `promote` walks upstreams to
  decide whether a promotion is legal, so a cycle is an unbounded walk
  inside a request handler. Reported through a sorted depth-first walk
  so the same graph always names the same cycle — an error message that
  varies between runs is a bad bug report, and there is a test that runs
  validation twenty times to hold that
- **cycles are only checked once the edges are known to exist**, so a
  typo'd upstream produces one confusing answer instead of two
- **a release gate nothing can reach is a fault.** Legal as a graph,
  useless as a workflow: it looks staged and can never produce a release
- **upstream order is presentation.** The same parents listed
  differently is not a re-parenting, and `is_noop` says so
- **occupancy is supplied, not queried.** The counts come from storage,
  the judgement does not — which is what keeps the whole thing testable

`gate_occupancy` on the storage trait, in both backends, counts bundles,
open publications above the window floor, and whether partition state
exists. Above the floor because those are the publications a fold still
reads, and therefore the ones a removed gate would strand — the shape of
finding 34. A missing partition row means a floor of zero rather than an
error, since a window that has never advanced has nothing below it.
Pinned in `backend_conformance`, so both backends have to agree on what
"still open" means.

## Next Task

Batch card 26.2 (server write path).
