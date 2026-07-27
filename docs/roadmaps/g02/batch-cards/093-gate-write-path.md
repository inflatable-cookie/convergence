# 093 Gate Write Path

Status: ready
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

## Next Task

Batch card 26.3 (CLI and TUI surface).
