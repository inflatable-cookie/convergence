# Batch 22.3 — Operator Guide Complete

Date: 2026-07-26
Roadmap: `g02.022`
Card: `089-operator-guide`

## Method

Built a real deployment, backed it up, destroyed it, restored it, and
checked what survived. The guide is written from what happened.

The deployment had the three things a restore has to bring back:
published work with provenance, a release channel, and a secret sealed
to a personal key. That last one is the point — doc 19 §1 concedes that
the server holds secrets it cannot read and therefore cannot regenerate,
which makes the backup the only mitigation that exists.

The restore worked. The secret decrypted, `verify` replayed the merge
and reproduced the bundle, and the release still materialized.

## Then The Interesting Part

Deleting only the `objects/` directory — the classic "I backed up the
database" mistake — produced this:

```
$ converge doctor
ok   server         reachable, credential accepted
ok   clock          0s from the server

nothing wrong here.
```

A deployment that could not hand over a single byte of any tree it had
ever stored, reporting healthy. Everything `doctor` asked touched the
control plane; nothing touched the object store.

For a batch whose whole job is "verify a restore", that is the gap.

**`converge doctor --deep`** adds one check: ask the server whether it
still holds the root manifest of its own `stable` release. One round
trip, no transfer, and precisely the question that fails when the
objects are gone. It stays opt-in so the ordinary run remains fast and
side-effect free.

## A Fetch Is Not A Restore Test

Worse, and quieter: against that same gutted deployment,

```
$ converge fetch --release stable --into /tmp/check
fetched bundle 9e67f65414b3 into /tmp/check
```

It wrote the correct tree. The workspace had fetched before, so it was
served out of its own local store — correct behaviour, and completely
useless as verification.

From a clean workspace the same command fails with a 404, as it should.

So the guide's verification procedure starts by making a throwaway
workspace, and the automated test uses a clean client with its own
`CONVERGE_HOME`. Anyone verifying a restore from the workspace they
already work in is testing their own cache.

## Two Backups, Not One

The server's data directory holds secret *ciphertext*. The keys that
open it live in `~/.converge` on each person's machine and never reach
the server — that is doc 19's threat model working as designed.

So losing either is total, in its own way, and neither is recoverable
from the other:

| Lost | Consequence |
| --- | --- |
| Server data directory | Everyone loses the ciphertext. |
| Your `~/.converge` | The ciphertext is fine; you cannot open it. |

The guide states this in a table rather than burying it in prose.

## Why "Stop The Server First" Is Real

SQLite here runs in `journal_mode = delete`, not WAL. A transaction in
flight leaves a `meta.sqlite-journal` beside the database, and a tar
that catches one without the other restores torn — discovered much later
than it happened. Checked rather than assumed.

## Pinned

`crates/converge-cli/tests/backup_restore.rs`:

- publish, release, seal a secret, copy the data directory, serve the
  copy, and assert the secret still decrypts, provenance still replays,
  and the tree still materializes
- the mistake case: database without objects, verified from a *clean*
  client, asserting plain `doctor` passes it and `--deep` does not

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: green
- every guide step run against a real deployment

## Next Task

Batch card 22.4 (real workspace shakedown), which the operator drives.
`22.5` (release) stays gated.
