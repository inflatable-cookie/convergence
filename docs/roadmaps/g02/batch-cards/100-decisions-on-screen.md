# 100 Decisions On Screen

Status: complete
Updated: 2026-07-29
Roadmap: `g02.027`

## Objective

The acts that replace the working tree are reachable from the TUI, and
safe there — not by asking less, but by asking on screen.

## Scope of the actual problem

Batch 27.4 gave the row views a working Enter and stopped at the
workspace boundary: Enter pulled a lane's objects into the store but
would not put them anywhere, because materializing needs the guard, and
the guard's whole output was a paragraph of prose telling somebody to go
and type a different command.

The operator's answer, 2026-07-29:

> But this is the point of the TUI — to make these complex actions
> accessible. "Because it's complicated" is a terrible reason not to do
> it. We need to find a solution where this works seamlessly, safely and
> cleanly.

Which is right, and the diagnosis was wrong. The obstacle was never the
complexity of the decision. It was that the decision existed only as a
string, and a string is not something a second surface can render.

Underneath that, `--force` meant two unrelated things:

- **uncaptured edits** live only in the working tree, so overwriting
  them destroys them and nothing brings them back
- a **diverged head** is the opposite: the snap record survives and
  `restore` returns to it whenever you like

One flag said yes to both, so anybody who had understood the recoverable
case was one keystroke from the unrecoverable one.

## In Scope

- `converge_model::overwrite`: the judgement as data — risks, whether
  each is recoverable, and the ways forward, with the labels both
  surfaces render
- `Workspace::overwrite_plan`, the one place the facts are gathered, for
  all three verbs that replace a tree
- `--preflight` on `restore`, `sync pull --materialize` and
  `fetch --checkout`: report and change nothing, the shape `gc` and
  `token prune` already use
- `--snap-first` on the same three: capture the tree, then overwrite —
  the option the CLI never offered
- the TUI decision screen, one key per option

## Out of Scope

- prompting when nothing is at risk. The clean case stays one keystroke;
  a prompt about nothing is how people learn to dismiss prompts

## Acceptance Criteria

- Enter on a lane brings it into the workspace, asking only when
  something is at stake
- the option that destroys unrecoverable work never borrows the language
  of the case that can be undone
- CLI refusal and TUI screen are generated from the same plan

## Outcome

Done, and driven. `k` (keep mine) is recommended in every case, because
it is the only option that costs nothing: your tree becomes a snap, that
snap is reachable forever, and you do not have to have learned what
`restore` is to be safe. The CLI's old refusal offered destroy or give
up; this one offers neither as the default.

Driving it found a bug the feature itself makes routine: `status`
measured pending changes against the newest snap *by timestamp*, not
against head. Those differ exactly when a snap sits off your current
line — which is what `--snap-first` creates every time — so a tree that
matched head byte for byte reported a pending change, permanently.
Fixed to measure against head.

Also found: `guard_overwrite` printed its "kept your work as" line with
`println!`, which inside the TUI lands on top of whatever ratatui has
drawn (a lane row rendered as `6hared`). The snap id is returned and put
in the caller's envelope instead, which is also how the TUI came to show
`kept 187b261f077a` in its result line.

## Notes

Fifth instance of the same structural lesson: **a rule with more than
one implementation will drift.** Three verbs replaced a working tree,
each with its own `--force` and its own refusal sentence, and the two
sentences did not agree about what `--force` meant. One plan, one
gatherer, three callers.
