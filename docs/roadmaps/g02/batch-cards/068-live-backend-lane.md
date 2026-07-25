# 068 Live Backend Lane

Status: complete
Updated: 2026-07-25
Roadmap: `g02.018`

## Objective

Make the external-backend conformance suite actually run — locally in
two commands, and nightly in CI — and close the two local test gaps the
roadmap listed: watch cadence and `.convergeignore`.

## Scope of the actual problem

The conformance suite has existed since batch 10.4 and had never been
run against a live service. Not because it was hard, but because an
unset env var made it print "skipping" and pass, so nothing ever
reported the gap. Meanwhile `.convergeignore` was tested only through
the dirstamp, and the watch loop's debounce — its entire reason for
existing — had no test at all.

## In Scope

- `CONVERGE_REQUIRE_BACKENDS`: a skip becomes a failure
- `effigy backends[:up|:test|:down]` over Effigy catalog services
- a nightly CI lane, guarded so it runs at most once a day and only
  after a day that produced commits
- watch cadence tests and `.convergeignore` behaviour tests

## Out Of Scope

- running the live lane on every push: it needs two services and several
  minutes, and the code it exercises changes rarely
- a repo-owned `docker-compose.yml`: Effigy's catalog services already
  describe Postgres and MinIO, and a second copy would drift

## Outcome

- **restore deleted ignored paths.** Materialize preserved `.converge`
  and `.git` and wiped everything else, so restoring a snap destroyed
  `build/`, caches, and local scratch — precisely the state a user
  marked as *not* project content. A snap never held those paths, so a
  restore has nothing to put back; deleting them is pure loss, and it is
  not what checking out a revision means anywhere else. Restore and
  bundle checkout now preserve root-level ignored entries
- the require-gate is the small change that matters most: skipping is
  now a decision rather than an accident, and a lane that forgets its
  feature flags fails instead of reporting success over an empty set
- the local lane is `effigy backends`, using catalog services rather
  than a compose file the repo would have to maintain
- CI runs nightly with a guard job: `workflow_dispatch` always runs, the
  schedule runs only when the last day produced commits. A green run
  over unchanged code is noise, and noise is what makes people stop
  reading a lane. The same guard carries the scale benchmarks from
  batch 15.4, which had been written and never scheduled — one nightly
  workflow, two lanes that are too heavy for every push
- MinIO runs as a step, not a service: it needs a `server /data`
  argument that the `services:` block cannot express
- watch tests pin the debounce (a settled tree captures once, an
  unchanged tree captures nothing, a moving tree does not capture per
  edit) and `.convergeignore` tests pin the documented *non*-features —
  `*.tmp` is a literal name, `!` lines are ignored, nested ignore files
  are data rather than configuration
- 200 tests green

## Next Task

Roadmap `g02.018` is complete, and with it the audit-hardening program
`g02.011`-`g02.018`. Next planning move: review the remaining backlog in
`docs/roadmaps/backlog/`.
