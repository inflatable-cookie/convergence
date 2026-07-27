# 093 Gate Write Path

Status: complete
Updated: 2026-07-27
Roadmap: `g02.026`

## Objective

Let an admin change a repo's gate graph, without a change landing on top
of work in flight.

## Scope of the actual problem

`set_gate_graph` exists in both backends and is called once, at repo
creation. Exposing it is three lines. Exposing it *safely* is the batch.

A graph change is read-modify-write against state that other requests
are using: `publish` resolves the target gate, `promote` walks upstreams,
`approve` records against a gate. A change that interleaves with any of
those can accept a promotion under a graph that no longer exists, or
refuse one under a graph that does not exist yet. Batch 13.1 settled how
this repo handles that — guarded batches through `apply_batch`, with the
loser retrying — and this belongs in the same shape.

The second problem is what to refuse. 26.1 answers "what would this
touch". This batch decides what that means: a change stranding a
partition, a bundle or an open publication is refused, and the refusal
carries the impact rather than a bare error.

## In Scope

- `PUT /api/repos/:repo/gates`, admin-only through `authorize_scoped`
- the change applied in one guarded batch, so it cannot interleave with a
  publish or promote
- refusal when the change would strand live state, naming what and where
- an explicit override for an operator who means it, since refusing
  outright would leave a repo that can never be reshaped
- `gate.changed` on the event feed, because a graph change is exactly the
  kind of thing another workspace needs to hear about

## Out Of Scope

- CLI and TUI (26.3)
- migrating live state between gates: this batch refuses or proceeds, it
  does not move bundles

## Design Notes

The override is the argument this batch has to get right. Refusing
unconditionally sounds safer and is not: a repo whose graph cannot be
reshaped because it once had a publication is a repo that has to be
recreated, which is worse than a documented sharp edge. The 20.4
precedent applies — warn, name the consequence, let the operator decide,
and never make the safe path the impossible one.

`Capability::Admin` already subsumes everything, so the authz work is
using `authorize_scoped` rather than inventing a capability. 21.4 found
what happens when a handler authorizes its own way.

## Acceptance Criteria

- an admin can reshape a graph; a non-admin cannot; a read-scoped token
  held by an admin cannot
- a change that would strand live state is refused with the impact named,
  and proceeds under the override
- a graph change concurrent with a publish leaves both consistent

## Validation

- `effigy validate`

## Outcome

`PUT /api/repos/:repo/gates`, admin-only through `authorize_repo`, with
three checks in front of the write — cheapest first, so a caller gets
the most specific refusal available:

1. is the graph legal at all (26.1 validation)
2. would the change strand work that exists
3. is it still the graph the caller read

`MetaOp::SetGateGraph` and `MetaOp::AssertGateGraph` in both backends,
so the write and its `gate.changed` event land in one guarded batch —
another workspace learning about a reshape that did not happen would be
worse than not learning. Graphs compare as parsed values rather than
text, because two encodings of the same graph are the same graph and a
whitespace difference should not look like somebody else's edit.

`expected` is optional. Sending it makes a concurrent edit lose loudly;
omitting it is allowed, because a script setting a known graph should
not have to round-trip first, and the cost is stated rather than hidden.

The `force` argument, settled: refusing outright sounds safer and is
not. A repo whose graph can never be reshaped because it once held a
publication is a repo that has to be recreated, which is worse than a
documented sharp edge. Batch 20.4 reached the same conclusion about
rotating after a departure — warn, name the consequence, let the
operator decide, and never make the safe path the impossible one. The
refusal names the gate and what it holds, and a `dry_run` reports the
same impact while changing nothing.

Seven tests, including two that exist because of specific past
failures: an admin's *read-scoped* token still cannot reshape the graph
(21.4 found twenty handlers that authorized their own way, one of which
let a read-scoped token grant itself admin), and a concurrent reshape
loses rather than silently overwriting the edit that beat it.

**Stated, not defended**: a publish that has already resolved its target
gate may complete under the previous graph. Closing that would mean
asserting the graph inside the publish batch, on the hot path, to defend
against a reshape that is refused anyway whenever the gate holds work.

## Next Task

Batch card 26.3 (CLI and TUI surface).
