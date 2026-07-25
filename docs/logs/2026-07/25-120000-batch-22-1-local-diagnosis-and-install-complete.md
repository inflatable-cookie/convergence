# Batch 22.1 — Local Diagnosis And Install Complete

Date: 2026-07-25
Roadmap: `g02.022`
Card: `087-local-diagnosis-and-install`

## Context: The Roadmap Was Reordered

The operator's position, given when this roadmap opened: *"Prep it, but
I'm not ready to release. I need to test thoroughly locally first."*

So `g02.022` was reordered around that. Local diagnosis and install
first, then the format guarantees that matter *before* real history
exists, then the operator story, then the shakedown — and the release
batch last, explicitly gated, not starting until the operator says so.

That ordering is also the better engineering. A release cut before a
real workspace has been used is a release of untested exposure. `g02.023`
made the case: every batch in it found defects by driving the real thing
that its own tests could not see. The point of doing the shakedown first
is to meet those defects while the only person affected is the one who
can fix them.

Nothing in this batch publishes anything.

## `converge doctor`

Reports workspace, personal key, remote, server reachability,
credential, access and clock skew — **all of them**, every run.

Stopping at the first failure would have reproduced exactly the problem
the verb exists to solve. Every verb already reports its own failure
correctly; what nobody has is the picture, and the failure you hit first
is rarely the first thing that is wrong.

Server state comes from one round trip. Reachability, then
authentication, then skew from three separate requests can describe a
state that never existed at any single moment.

Clock skew is measured against the server's own `Date` header, with half
the round trip charged to the server's favour so a slow link does not
read as a wrong clock. Past 60 seconds it is a failure rather than a
note, matched to the identity exchange's leeway from batch 21.3: further
out, a provider-issued token is refused and the refusal blames the
token, which sends someone looking in entirely the wrong place.

It changes nothing. A test walks the workspace and `CONVERGE_HOME`
before and after to prove it — a diagnostic you cannot safely run when
you are unsure is not one.

## Driving It Found A Wrong Fix Line

The first version told a bootstrap admin whose repo did not exist yet to
"ask an admin: converge member add". They *are* the admin. The real
answer was `converge repo create`.

The server answers "no such repo" and "you cannot read this repo"
identically, on purpose — whether a repo exists is itself privileged.
So `doctor` cannot tell them apart either, and guessing produced a dead
end. The fix now names both possibilities and says why.

A diagnostic that confidently recommends the wrong command is worse than
one that says less.

## A Pre-Existing Output-Contract Bug

`doctor` wanted to print a report and exit non-zero. Writing that
surfaced the same shape already shipped in two places: `verify` and
`resolve validate` both `emit` a report and then `bail!`, so under
`--json` they printed **two** envelopes. Anything reading one line per
command got a parse error instead of a result.

`ReportedFailure` now means "already printed, just set the exit code".
All three use it. The outer envelope stays `ok: true` — the command ran
— and the answer lives in the report, which is what the envelope
contract always said.

## Version And Install

`converge --version` now carries the commit it was built from:

```
converge 0.1.0 (v0-legacy-93-g40d945e-dirty)
```

`0.1.0` moves rarely pre-1.0, so a note about "0.1.0 behaviour" names a
moving target. The describe string does not, and `-dirty` says the tree
had uncommitted changes. A `build.rs` produces it, falling back to
`unknown` without git rather than failing the build.

`docs/guides/004-running-it-locally.md` covers install, standing up a
server, first workspace, diagnosis, secrets, backup and teardown. Its
install path was run rather than written from memory.

Its backup section is deliberately marked as untested: "a backup you
have not restored is a hypothesis". Batch 22.3 turns it into a
procedure.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 280 passed, 4 skipped
- `cargo install --path` run for real; every `doctor` state driven
  against a live server, including revoked credentials and a stopped one

## Next Task

Batch card 22.2 (store format and upgrade refusal), which lands before
the shakedown so the first store with real history in it has the guard.
