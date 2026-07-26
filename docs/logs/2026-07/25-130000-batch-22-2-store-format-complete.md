# Batch 22.2 — Store Format And Upgrade Refusal Complete

Date: 2026-07-25
Roadmap: `g02.022`
Card: `088-store-format-and-upgrade`

## Why This Came Before The Shakedown

Batch 22.4 exists to accumulate real local history. The moment that
history exists, "pre-1.0, just re-init" stops being an acceptable answer
— and adding the guard afterwards would mean the first store that needed
it did not have it.

The failure being prevented is not a crash. A crash would be fine. It is
a newer binary **silently misreading** an older store: a field that
gained a meaning, an id whose domain tag changed (batch 18.3 moved
`converge-snap-v3` to `v4`), an enum that gained a variant an old reader
skips. Those corrupt quietly, and the corruption surfaces long after the
thing that caused it.

## The Existing Version Field Could Not Have Worked

`WorkspaceConfig` has carried `version: 1` since the rebuild, and
nothing ever read it. That is worse than having none, because it looks
like a guard.

It also could not have worked as one. `config.json` is parsed by serde,
so a format change that alters its *shape* fails to parse before
anything gets to look at the version. The error would have been "missing
field", not "wrong version".

A version stamp has to be readable by every binary that will ever meet
it, including ones written after the format it stamps. So it is a
standalone file holding one line: `.converge/format` for a workspace,
`format` in a server's data directory.

## Absent Means 1

A store written before the stamp has none. Absent is version 1,
permanently, and nothing rewrites it — so opening a store stays a pure
read.

That property is load-bearing rather than tidy. Batch 22.1's `doctor`
opens a workspace and is tested to change nothing; a migrate-on-open
would have quietly made the diagnostic a mutation.

## `--force` Was A Hole

Driving it found the thing worth finding. Every verb correctly refused a
format-99 workspace — `status`, `history`, `snap`, `publish`, all of
them. Then:

```
$ converge init --force
initialized workspace at /tmp/fmt/ws
$ cat .converge/format
converge-workspace-1
```

`init --force` cheerfully destroyed the store and reset it to format 1,
discarding exactly the history the refusal existed to protect.

Worse, batch 22.1's own `doctor` was pointing there: its fix line for a
failed workspace check read "converge init (or cd into a workspace)".
The diagnostic was recommending the one command that would destroy the
thing.

Both fixed. `--force` means "re-initialise over my own store", not
"destroy one I cannot read" — discarding an unreadable store now means
removing the directory yourself, which is an unmistakable act rather
than a flag people reach for casually. And `doctor` says `do NOT run
init --force here`.

## Both Directions

An older binary opening a newer store is the more dangerous case: it
reads fields whose meaning changed underneath it. It is also the one
people hit, because downgrading is what you do when a new version
misbehaves. Both are refused, on *open*, before anything is read — so
the refusal can say "Nothing has been read or written", and mean it.

A stamp naming the wrong *kind* gets its own message: a workspace passed
where a data directory belongs is a different mistake with a different
fix than a version mismatch.

## Policy

Doc 16 §3 records what requires a bump. The test is **would a binary at
the other version misread this**, not "did the bytes change". Adding a
file nobody older looks for is not a bump; changing what an existing one
means is.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 291 passed, 4 skipped
- driven: stamped a workspace to format 7 and confirmed every verb
  refused, `init --force` refused, `doctor` reported it without
  recommending destruction, and the server refused a format-9 data
  directory

## Next Task

Batch card 22.3 (operator guide), which turns guide 004 §6's untested
backup paragraph into a procedure with a verified restore.
