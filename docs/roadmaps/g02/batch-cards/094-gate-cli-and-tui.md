# 094 Gate CLI And TUI

Status: ready
Updated: 2026-07-27
Roadmap: `g02.026`

## Objective

Give a person the verbs to shape a gate graph, and show them what a
change does before it does it.

## Scope of the actual problem

`converge gates` reads the graph and that is all it does. `converge repo`
has one subcommand, `create`. So the surface for this does not exist
rather than being wrong.

The shape is settled by what the last two batches proved. `gc` and
`token prune` report by default and act on `--execute`, and both caught
a real defect precisely because a dry run was the default — the first
staleness check in `token prune` classified the one live credential on
the machine as dead. A gate edit is at least as consequential.

The refusal shape is settled too. Finding 30's diverged-pull message
landed on: what is affected, what keeps it safe, the command that
proceeds anyway. And finding 32 is worth remembering while writing any
of this output — printing `<name>` where the real name is known is the
kind of thing that survives review because nobody runs the line.

## In Scope

- `converge gates add <id> [--upstream <id>]... [--approvals N]
  [--strategy S] [--releasable]`
- `converge gates edit <id>` with the same flags, and `gates rm <id>`
- `converge gates set --file <graph.json>` for the whole graph at once,
  because a multi-gate reshape as a sequence of single edits passes
  through states that are not legal
- report by default, apply on `--execute`, in `gc`'s idiom
- impact rendered for a person: which gates, how many bundles, how many
  publications in an open window
- `--json` envelopes for all of it
- TUI: the gate-graph editing 23.3 deferred, in the wizard shape 23.3
  established

## Out Of Scope

- moving live state between gates
- a graph designer beyond text: the TUI wizard edits one gate at a time,
  and `gates set` is the escape hatch for a wholesale reshape

## Design Notes

`gates set --file` earns its place: adding a review gate between intake
and release means intake's downstream and release's upstream change
together, and any single-step ordering passes through a graph that
validation would reject. One atomic submission avoids inventing a
"temporarily illegal" state that everything else then has to tolerate.

Whether `--execute` or a confirmation prompt is right in the TUI is
23.3's question, already answered there: the review step *is* the
confirmation, and its legend names the consequence.

## Acceptance Criteria

- a staged graph can be built from a fresh repo with documented commands
- a destructive change reports its impact and does nothing without
  `--execute`
- every printed remedy is runnable as printed
- the TUI can edit a gate without dropping to a shell

## Validation

- `effigy validate`
- `effigy qa:docs`

## Next Task

Batch card 26.4 (multi-gate end to end).
