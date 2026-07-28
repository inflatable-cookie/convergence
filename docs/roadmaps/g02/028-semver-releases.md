# 028 Semver Releases, Channels Retired

Status: complete
Owner: repo maintainers
Updated: 2026-07-28

## Context

The releases tile during 27.3 showed the problem in one screenshot:
three releases, all named `stable`, distinguishable only by bundle id.
No order, no meaning. The operator asked whether to enforce semver, and
then the sharper question: is there any use case for channels at all?

In this product, no — and the reason is specific to Convergence.
Channels elsewhere do three jobs. Pre-release tracks are semver
prerelease tags (`1.2.0-beta.1` *is* the beta track). Staged promotion
is what **gates** already do — a channel is a second promotion ladder
bolted beside the first, and it was the operator's "one concept too
many". Rollback-by-repointing is the one genuine gap, and guide 005
already rejects it: fix forward, never re-point. A `yank` flag covers
withdrawal more honestly than a movable pointer ever could.

## Decisions (operator, 2026-07-28)

- **Semver is the release identity.** A release is `<bundle> as v1.2.0`.
  Unique per repo, immutable, enforced server-side in the guarded batch
  so two racing releases cannot both claim a version.
- **Backports work by default.** Versions are *unique*, not strictly
  increasing: cutting `1.1.1` while `2.0.0` exists is how long-term
  support works, and forbidding it "would be a deal breaker for most
  people". Strictly-increasing is a later opt-in gate policy, not the
  default.
- **Existing releases get real numbers, not a legacy label.** Semver is
  supposed to be enforced; a permanent "unversioned" caste contradicts
  the rule the feature exists to state. Migration assigns `0.<n>.0` by
  release order (`seq`), deterministically, on server open — the same
  open-time migration shape `object_pins.pinned_at` used.
- **`latest` is a computation, never a pointer**: highest non-yanked,
  non-prerelease version. `--release latest`, `--release 1.2.0`, and
  range forms (`--release 1.x`) replace `--release <channel>`.
- **Yank, not delete**: a yanked release stays in history, marked,
  excluded from `latest` and from range resolution unless named exactly.

## Execution Plan

- **28.1 Version model** (card 100): semver parsing/ordering in
  `converge-model` (the `semver` crate — prerelease ordering is exactly
  the kind of thing not to hand-roll), release-version rules as pure
  functions: uniqueness, `latest`, range matching, yank filtering.
- **28.2 Server** (card 101): `ReleaseRecord` carries `version` and
  `yanked`; release verb validates and guards uniqueness in the batch;
  open-time migration numbering existing rows `0.<n>.0`; yank route;
  GC protects non-yanked releases; retention `keep_releases_per_channel`
  becomes `keep_releases`.
- **28.3 CLI and TUI** (card 102): `converge release <bundle> --as
  1.2.0`, `converge yank <version> --reason`, `releases` sorted by
  version with yanks marked, `fetch/bundle/verify --release
  latest|<version>|<range>`, the TUI releases tile and view.
- **28.4 Drive it** (card 103): on the real repo — the migration
  numbering the three existing releases, a normal release, a backport
  below `latest`, a yank and what `latest` then answers, and the
  channel verbs' error messages pointing at the new forms.

## Exit Criteria

- a release cannot exist without a valid, unique semver version
- the three pre-semver releases on the shakedown repo carry real numbers
- a backport below `latest` succeeds by default; `latest` is unchanged by it
- a yanked release leaves `latest` and ranges but stays in history
- no verb, view, or doc mentions channels except the migration note

## Outcome

All four batches landed (28.2–28.4 in one sweep so main stayed green —
changing `ReleaseRecord` alone breaks every consumer). Proven on the
live deployment: the migration numbered the three pre-semver releases
v0.1.0–v0.3.0, a duplicate was refused with the fix-forward message, a
backport landed below `latest` without moving it, and a yanked 1.0.0
left `latest` while `verify --release 1.0.0` still replayed it.

The finding that earns the log entry: minutes after the whole suite
passed, the real deployment refused to release —
`NOT NULL constraint failed: releases.channel`. The legacy column only
exists on *migrated* databases; every fresh fixture builds the new
schema. The migration now drops the column, and a regression test
builds the legacy schema by hand and releases onto it. The
fresh-fixture blind spot, second appearance, this time in schema shape.

Deliberately kept: `--channel` as a hidden alias for `--as` (muscle
memory is not a defect), and doc 18's release refs marked Deferred —
only lane refs export today.

## Next Task

`g02.027` continues (27.2 guidance, 27.4 the operator drives it cold).
`22.5` remains the operator's call.
