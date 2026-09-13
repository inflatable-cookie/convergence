# Flattened Task Switchover Closeout

Date: 2026-09-09
Handoff: `20260909-140500-flattened-task-switchover.md` (consumed at closeout; durable record is [PR #5](https://github.com/inflatable-cookie/convergence/pull/5))
Status: complete; merged in PR #5

## Outcome

The one-time Northstar flattened-task migration merged into `main` as
`bb978644962cddda8787f512b829fc54806273fa`. Worker head
`7fbc438d214ba90329ae0813d6b477f07c92fc74` was merged without follow-up
changes.

The merged documentation migration:

- compacted safely closed `g01` into `docs/roadmaps/archive/g01.md`;
- made each `docs/roadmaps/g02/NNN-<slug>.md` the sole `g02.NNN` task;
- absorbed card 091's open release state into `g02.022`;
- removed the manifest-listed `docs/roadmaps/g02/batch-cards/` tree;
- refreshed the roadmap, generation, task, log, contract, instruction, and
  specification front doors; and
- preserved the open product commitments, historical terminology, evidence
  pointers, and retained numbering exceptions.

## Acceptance and review

The accepted review at the exact worker head is recorded in
[PR comment 5602607621](https://github.com/inflatable-cookie/convergence/pull/5#issuecomment-5602607621).
It found no blocking findings and verified manifest conformance, front-door
agreement, unique task IDs, retained exceptions, and scope boundaries.

## Validation

The accepted review reports the full merge gate at the worker head:

- `effigy validate`: fmt, check, clippy, and 364/364 nextest tests passed;
- `effigy qa:docs`: passed;
- `effigy health`: passed; and
- `git diff --check`: clean.

The post-merge closeout reran `effigy qa:docs`, `effigy health`, and
`git diff --check` on the synchronized integration checkout. The deletion
audit still has 101 deletions, all under the manifest-listed batch-card tree;
there are no out-of-scope deletions.

## Deferred state and next pointer

No migration failure is deferred. The migration's declared historical
exceptions remain: closed records and immutable history may still mention old
card numbering, while no active executable surface depends on the removed
batch-card tree.

The product frontier is unchanged. Planning direction is still needed among:

- `g02.027` cold-drive closeout;
- the operator-gated `g02.022` release cut; or
- a bounded audit follow-up, which would become `g02.032` only after
  promotion.

`g02.024` and `g02.025` remain parked on their triggers. Normal dispatch does
not resume until the operator selects the next product direction.

## Next Task

None created by this closeout. The next move is the existing operator intent
checkpoint in `docs/roadmaps/g02/README.md`.
