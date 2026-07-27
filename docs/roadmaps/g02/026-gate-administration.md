# 026 Gate Administration

Status: complete
Owner: repo maintainers
Updated: 2026-07-27

## Context

`converge repo create` provisions one gate, `intake`, and nothing can
change the gate graph afterwards. There is no CLI verb, and
`/api/repos/:repo/gates` is `get` only — `set_gate_graph` is called
exactly once, at repo creation.

So `promote`, one of the six verbs the contract is built around, cannot
be reached by any user. Batch 22.4 found this by trying to use it
(finding 33):

```
$ converge promote cb59de7525b6 --to intake
gate intake does not accept promotions from intake
```

The refusal is correct. There is simply no downstream gate to name
instead, and no way to make one. Everything the multi-gate design
describes — staged review, required approvals, a release-only final
gate — is implemented server-side and unreachable.

The operator's position, 2026-07-27: *"We don't ship until Convergence
works consistently without issues for all use cases end to end."* This
roadmap is the gap between that and where 22.4 left things, and `22.5`
does not release until it closes.

## Findings Addressed

- **22.4 finding 33**: the gate graph is write-once at repo creation, so
  `promote` is unreachable and the multi-gate design is unusable
- **22.4 finding 34**, structurally: retention wedged a gate permanently
  in part *because* a single-gate repo cannot promote, so its window
  never advances and poisoned publications never age out. Gate
  administration removes the condition that made that failure permanent
  rather than merely annoying
- **22.4 finding 10**: a single-gate repo can never GC published
  objects, for the same reason — nothing ever leaves the window

## The actual problem

The write path is the easy half: a route, a verb, and validation that
the graph is a graph. What makes this a roadmap rather than a card is
what a graph change does to state that already exists.

Gates are not configuration sitting beside the data. They are part of
its addressing:

- partition state is keyed `(repo, scope, gate)` and carries the window
  floor and base bundle
- bundles belong to a gate, and their windows advance per gate
- publications target a gate and sit in its open window until it advances
- approvals are recorded against a bundle at a gate

Remove a gate and every one of those becomes unreachable — the same
shape as finding 34, where a dangling reference wedged a partition with
no way back through the CLI. Re-parent a gate and promotions that were
legal yesterday are not today, while bundles promoted under the old
shape stay where they are.

So the design question is not "how do we write the graph" but **what a
change is allowed to do to work already in flight**, and how somebody
finds out before rather than after.

## Design commitments

Three, taken from what the last two batches proved:

1. **Report before you change.** `gc` and `token prune` are dry by
   default, and both caught a real defect because of it. A graph edit
   shows its blast radius — which partitions, how many bundles, how many
   publications in an open window — and applies only when asked.
2. **Refuse rather than orphan.** A change that would strand live state
   is refused by default and names what it would strand, in the shape
   finding 30's diverged-pull refusal settled on: what is affected, what
   keeps it, the command that proceeds anyway.
3. **Additive changes stay easy.** Adding a gate and pointing it at an
   existing upstream orphans nothing, and should not need ceremony. The
   care is for removal and re-parenting.

## Execution Plan (batch details in cards)

- **26.1 Graph model and impact analysis** (card 092): validation as a
  pure function — cycles, unknown upstreams, at least one entry gate,
  known strategies, `may_release` reachable — plus the impact query that
  answers "what does this change touch", both testable without a server
- **26.2 Server write path** (card 093): `PUT /api/repos/:repo/gates`,
  admin-only through `authorize_scoped`, applied in one guarded batch so
  a graph change cannot interleave with a publish or a promote; refusal
  carries the impact rather than a bare error
- **26.3 CLI and TUI surface** (card 094): `converge gates add|edit|rm`
  and a whole-graph `set`, reporting by default and applying with
  `--execute`, the refusal naming the override; gate graph editing in
  the TUI, which 23.3 deferred
- **26.4 Multi-gate end to end** (card 095): the batch this roadmap
  exists for. A real repo with a staged graph, driven the way 22.4 drove
  everything else — publish into intake, approve, promote to review,
  promote to release, release from the gate that may. Then the
  adversarial half: a graph change racing a publish, removing a gate
  with live bundles, re-parenting mid-flow, and whether a window that
  can now advance actually lets GC reclaim (findings 10 and 34)

## Exit Criteria

- a repo can be given a staged gate graph after creation, and changed
  again later
- `promote` works end to end on a real multi-gate repo, with
  `required_approvals` enforced and `may_release` respected
- no graph change can strand a partition, a bundle or a publication
  without saying so first and being asked twice
- a window that advances lets GC reclaim what it could not before
  (findings 10 and 34 checked, not assumed)
- the guides describe the multi-gate flow, because until now nobody
  could follow them

## Outcome

All four batches complete. A repo can be given a staged gate graph after
creation and changed again later; `promote` works end to end with
`required_approvals` enforced and `may_release` respected; no graph
change can strand a partition, a bundle or a publication without saying
so first and being asked twice.

The batch that mattered was the last one. 26.1 to 26.3 built the surface
and found ordinary things. 26.4 drove it and found four defects that no
test could have seen, because no repo had ever had a second gate — three
of them the same assumption in different clothes, that a bundle is only
ever at the gate that produced it. A staged pipeline could be described
and never walked.

The measurement is in the 26.4 log: before any promotion the window floor
is 0, so no publication is eligible for collection whatever the retention
policy says. Gate administration is what unblocks findings 10 and 34, and
that is now a number rather than an argument.

## Next Task

`22.5` (release) becomes the operator's call again.
