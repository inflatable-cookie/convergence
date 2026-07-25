# 073 Consumption

Status: complete
Updated: 2026-07-25
Roadmap: `g02.019`

## Objective

Get a secret to the process that needs it without writing plaintext into
the working tree, and make every read visible afterwards (doc 19 §10).

## Scope of the actual problem

A stored secret nobody can use is a filing cabinet. Most secrets end up
in an environment variable, and the usual route — a `.env` file — is a
bad fit for a repository an AI agent works in: the agent reads the tree,
and the credential lands in a model context or a commit.

The boundary that actually fixes that is an identity, not a file format
(doc 19 §10a): an agent running under a subject without the `secret`
capability cannot reach a credential however it looks around. This batch
builds the delivery paths that make the boundary usable, and the audit
trail that makes a breach bounded.

## In Scope

- `converge run --secret NAME -- cmd` — named secrets into one child's
  environment, nothing at rest, exit code propagated
- `converge secret write-env PATH` — the escape hatch, made loud:
  warns, adds the path to `.convergeignore`, audits
- `secret.read` events on every fetch that enables decryption
- redaction: a secret value must never reach the TUI's Last strip or the
  agent trace

## Out Of Scope

- process supervision (doc 19 §9): `run` starts one child and gets out
  of the way
- non-environment injection — file descriptors, `systemd` credentials —
  until a consuming program supports one

## Acceptance Criteria

- a secret reaches a child process without touching the working tree;
  `write-env` self-ignores and warns; reads appear on the events feed;
  no surface prints a decrypted value; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge run --secret NAME -- cmd` injects named secrets into one
  child and propagates its exit code, which is the whole reason to run
  it. `--secret ENV=NAME` maps when the names differ; otherwise
  `db-password` becomes `DB_PASSWORD`, predictable enough that nobody
  looks it up
- `secret write-env` **closes the door behind itself**: it warns, writes
  `0600`, shell-quotes so a value with a space survives, and adds the
  path to `.convergeignore` — a plaintext dotenv captured into a snap
  would be the exact leak this roadmap exists to prevent
- the escape hatch points at `converge run` in its own output. An option
  documented as weakest should say so where someone is using it, not
  only in a guide
- `secret.read` events on every fetch that enables decryption. The
  server learns when each person uses each secret, which doc 19 §10c
  chose over read-privacy
- redaction happens where results are **formatted**, not at each call
  site, so a new surface cannot forget: the Last strip prints
  `(secret value withheld)` for `secret get` and `run`
- the agent trace already recorded only argv and outcome, never
  payloads — so it was safe by construction rather than by patch. A test
  now pins that, because it is load-bearing and easy to break later
- 224 tests green

## Next Task

Roadmap `g02.019` is complete. Open `g02.020` (shared secrets).
