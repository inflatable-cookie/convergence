# Working rules

How Convergence executes work. Tasks, briefs, status and outcomes live in
Queue, never in this repository.

## Execution

- Start through `docs/README.md`, `docs/knowledge/README.md`, and
  `docs/plan.md`.
- Route commands through Effigy: `effigy tasks`, `effigy doctor` when routing
  or health is unclear, `effigy test --plan` before choosing test scope.
- Keep a change bounded to one honest owner. Do not mix research expansion,
  platform invention, and UX polish into one lane.
- When the next direction is materially ambiguous, stop and ask. For
  Convergence that usually means naming whether the move is object-model
  work, gate or authority workflow, operator/bootstrap work, or continued
  planning.
- Update code, knowledge, tests, and indexes together when they form one
  observable change.
- File papercuts in Queue with `papercut.add` (see the `northstar`
  skill); there is no repository papercuts file.

## Product facts every change must respect

Load-bearing vocabulary and hashed contracts live in `AGENTS.md` and
[architecture](../architecture/README.md). Link to them.

- Where code and knowledge disagree, knowledge wins until a decision moves
  both.
- Secret values never enter the TUI: the input buffer echoes, submitted lines
  replay, and the trace outlives the session. Those verbs hand over to a shell.

## Release

Implementation approval does not authorize a tag. Cutting a release needs
explicit operator authority. The procedure is [release.md](release.md).

## Definition of done

A change is done when its scoped behaviour is implemented, the repository QA
that covers it passes, and the owning knowledge files match the result.
Remaining gates are named, not implied complete.
