# 006 Staged Gates

A repo starts with one gate, `intake`, and work published there can be
released straight away. That is the right shape for one person, and the
wrong one the moment somebody wants review before release.

This is how to add stages, and what each one does.

Everything below was run against a real repo while batch 26.4 was
written; nothing here is aspirational.

## What a gate is

A gate holds a window of publications and produces bundles from them. A
gate graph says which gates accept promotions from which, how many
approvals a bundle needs before it can leave, and which gates may
release.

```
converge gates
```

```
intake  entry  0 approval(s)  whole-file
review  after intake  1 approval(s)  whole-file
release  after review  0 approval(s)  whole-file  releasable
```

- **entry** means publications land here. At least one gate must be an
  entry gate, or there is nowhere to publish
- **approvals** are what a bundle needs before it can be promoted *out
  of* that gate. `review`'s 1 is what makes review mean anything
- **releasable** is where `converge release` may be run from

## Building one

Report first, apply with `--execute` — the same shape as `gc`:

```
converge gates add review --upstream intake --approvals 1
converge gates add review --upstream intake --approvals 1 --execute
```

For a reshape that touches more than one gate, use a file. Inserting a
stage between two existing gates changes both their edges, and no
ordering of single edits stays legal throughout:

```json
{
  "gates": [
    {"gate_id": "intake",  "name": "Intake",  "upstreams": [],
     "required_approvals": 0, "strategy": "whole-file", "may_release": false},
    {"gate_id": "review",  "name": "Review",  "upstreams": ["intake"],
     "required_approvals": 1, "strategy": "whole-file", "may_release": false},
    {"gate_id": "release", "name": "Release", "upstreams": ["review"],
     "required_approvals": 0, "strategy": "whole-file", "may_release": true}
  ]
}
```

```
converge gates set --file graph.json --execute
```

An illegal graph is refused with every reason at once, not the first:
unknown upstreams, cycles, no entry gate, an unknown strategy, or a
release gate nothing can reach.

## Walking it

```
converge publish                            # lands in intake
converge promote <bundle> --to review       # intake requires no approval
converge approve <bundle>
converge promote <bundle> --to release      # review requires one
converge release <bundle> --channel stable
```

Skipping a stage is refused: `release` accepts promotions from `review`,
so a bundle has to have reached review first.

A bundle keeps the gate that produced it for ever. What moves is where
it has *reached*, which is what promotion records — so `promote` and
`release` both ask which gates the bundle has been through, not which
one built it.

## Changing a graph that is already in use

Removing or re-parenting a gate can leave bundles and publications that
nothing addresses. That is refused, with what it holds:

```
$ converge gates rm intake
remove  intake
move    review: intake -> entry
  intake holds 8 bundle(s) and 13 open publication(s)

this would strand work that nothing else addresses.
promote or release it first, or add --force.
nothing changed. re-run with --execute.
```

`--force` proceeds. It exists because a repo whose graph can never be
reshaped once it has held a publication would have to be recreated
instead, which is worse than a sharp edge you were warned about. The
work stays in the store either way; what it loses is anything that can
reach it.

Adding a gate strands nothing, so it needs no ceremony — and it is the
only graph change on a keystroke in the TUI (`a` on the gate screen).

## Why staging affects storage

A gate's window only advances when a bundle is promoted out of it. Until
then every publication stays open, and open publications are never
collected — whatever the retention policy says.

A single-gate repo therefore accumulates: it has nowhere to promote to,
so its window never advances, so `converge gc` can never reclaim
published objects. Measured on the shakedown repo, the first promotion
moved the floor from 0 to 14 and made 14 publications and 33 objects
collectable that had not been before.

This is not a reason to add stages you do not want. It is a reason to
know that a single gate is a choice with a cost, rather than the only
possibility.

## Next Task

None. This describes a procedure.
