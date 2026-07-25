# 087 Local Diagnosis And Install

Status: complete
Updated: 2026-07-25
Roadmap: `g02.022`

## Objective

Make it possible to run Convergence on your own machine, and to find out
why it is not working when it is not working.

## Scope of the actual problem

Today the only way in is `cargo build` from a clone, and the only way to
diagnose a broken setup is to run verbs until one of them says something
useful. Each verb reports its own failure correctly — `converge publish`
says "no remote configured", `secret get` says "no personal key on this
machine" — but nobody debugging holds all of those in their head, and
the failure you hit is rarely the first thing that is wrong.

`converge doctor` exists to answer one question: *what is the state of
this setup, and what is wrong with it.* Not a health score. A list of
facts, and for each broken fact the command that fixes it.

The specific things a first real session gets wrong, from driving this
repo across `g02.023`:

- no workspace in this directory (every verb fails, none says "you are
  in the wrong directory")
- a remote configured against a server that is not running
- a token that has expired or been revoked — 21.1 made those distinct
  messages, but only if you happen to run a remote verb
- no personal key, which turns every secret verb into the same error
- clock skew against the server, which 21.3 found the hard way: an
  identity token 60 seconds out is refused, and the refusal blames the
  token

## In Scope

- `converge doctor`, human and `--json`, reporting workspace, remote,
  identity, key, and clock state
- every failing check names the command that fixes it
- exit status: non-zero when something is actually broken, so it is
  usable in a script
- `converge --version` reporting a commit, not just a crate version
- a documented local install (`cargo install --path`), and a documented
  throwaway local deployment for exercising the whole thing

## Out Of Scope

- **anything that publishes.** No release workflow, no tags, no
  packaging, no `.github/` changes. That is 22.5, and it is gated
- auto-fixing. `doctor` reports and recommends; a diagnostic that
  changes state is one you cannot run when you are unsure

## Acceptance Criteria

- `doctor` on a broken setup names every problem and its fix, not just
  the first; on a healthy one it says so and exits zero; `--json` is
  consumable; a guide exists that stands up a local deployment from
  nothing

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge doctor` reports workspace, personal key, remote, server
  reachability, credential, access and clock skew — **all of them**,
  every run. Stopping at the first failure would have reproduced exactly
  the problem it exists to solve
- server state comes from **one** round trip. Reachability, then
  authentication, then skew from three separate requests can describe a
  state that never existed at one moment
- clock skew is measured against the server's own `Date` header, with
  half the round trip charged to the server's favour so a slow link does
  not read as a wrong clock. Past 60 seconds it is a failure, matched to
  the identity exchange's leeway (batch 21.3): further out, a
  provider-issued token is refused and *the refusal blames the token*
- **driving it found a wrong fix line.** A bootstrap admin whose repo did
  not exist yet was told to ask an admin for `member add`, which is a
  dead end. The server answers "no such repo" and "you cannot read this
  repo" identically on purpose — existence is privileged — so the fix
  now names both possibilities instead of guessing
- `doctor` changes nothing, and a test walks the workspace and
  `CONVERGE_HOME` before and after to prove it. A diagnostic you cannot
  safely run when unsure is not one
- **fixed a pre-existing output-contract bug it shared.** `verify` and
  `resolve validate` emit a report and then `bail!`, so `--json` printed
  *two* envelopes and anything reading one line per command got a parse
  error instead of a result. `ReportedFailure` now means "already
  printed, just set the exit code"; all three use it
- `converge --version` carries the commit it was built from. `0.1.0`
  moves rarely pre-1.0, so a bug report against it names a moving target
- `docs/guides/004-running-it-locally.md`: install, stand up a server,
  first workspace, diagnose, secrets, back up, throw away. Its install
  path was run rather than written from memory
- 280 tests green

## Next Task

Batch card 22.2 (store format and upgrade refusal).
