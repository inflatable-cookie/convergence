# 029 Candidate Rename

Status: complete
Owner: repo maintainers
Updated: 2026-07-29

## Context

The operator, reading the newly-titled rows: "bundle" doesn't describe
what the thing is — "an amalgamation of snaps put forward as a
submission to the next gate." The word named the shape (stuff
amalgamated) where a newcomer needs the role named (a thing seeking
passage through gates).

**Candidate** won on lifecycle fit: a candidate at intake, a candidate
awaiting approval at review, a release candidate at the release gate —
borrowed vocabulary every developer already owns, the same
familiarity win semver gave releases. The known wrinkle is accepted:
"candidate" implies it might be rejected and there is no reject verb; a
candidate that fails review sits until the next window supersedes it,
and that is the answer.

## Decision

Full rename, operator's call, done before 22.5 ships because pre-release
is the only cheap moment — after a release it is frozen vocabulary. A
surface-only rename was rejected by the project's own record: a concept
with two names in one codebase is the drift trap documented four times
this generation.

## What moved

- every Rust identifier, wire type and route (1283 occurrences, 59 files)
- storage: `bundles` → `candidates`, `bundle_id`/`base_bundle_id`
  columns renamed by open-time migration in both backends; the empty
  `candidates` table the same open creates must not shadow legacy data
- persisted JSON reads through serde aliases (`bundle_id`,
  `base_bundle_id`, `derived_from_bundle`, `last_seen_bundle`) and
  re-serializes with the new names on next write
- CLI: `converge candidate`, with `bundle` as an alias; TUI nav key `c`,
  with `b` kept for muscle memory; git export trailer is now
  `Converge-Derived-From-Candidate`
- living docs and the AGENTS.md terminology rule; logs and closed cards
  stay as written, because history is a record, not documentation

Proven: the migration test builds a bundle-era schema by hand and opens
it; a copy of the live deployment migrated with all eleven candidates
and their approvals intact.

## Next Task

None — single-sweep rename. `22.5` remains the operator's call.
