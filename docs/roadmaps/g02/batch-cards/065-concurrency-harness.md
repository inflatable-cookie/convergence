# 065 Concurrency Harness

Status: complete
Updated: 2026-07-25
Roadmap: `g02.018`

## Objective

Stand up a multi-writer test rig and drive the transactional guards from
batches 13.1-13.2 with real clients on real threads. The audit found
zero concurrency tests, which is how every race it reported shipped.

## Scope of the actual problem

The guards were fixed and tested single-threaded: a hand-built
interleaving proves the check exists, not that the check holds when two
requests genuinely overlap. And two interleavings were never exercised
at all — promotion racing publication (promotion moves the floor other
publishes are reading), and GC racing an upload (batch 12.2's pin).

## In Scope

- a `Cluster` harness: server, N tokens, two gates, one partition
- publishes racing promotions, asserting window contiguity
- simultaneous promotion of one bundle
- GC running continuously against in-flight chunked uploads

## Out Of Scope

- failure injection (18.2), property tests (18.3), live backends (18.4)
- loom or a deterministic scheduler: these tests exercise the *server's*
  transaction boundaries through HTTP, where the interleaving that
  matters is between requests, not between memory operations

## Outcome

- **the harness found a real defect on its first run**: promoting the
  same bundle into the same gate twice both succeeded and recorded two
  promotions. The state half was already idempotent (`is_current_w`
  skips the advance), so this was a duplicate row, not a corrupted
  floor — but promotion history is provenance, and provenance that
  double-counts is wrong
- fixed by making promote genuinely idempotent: a promotion into a gate
  the bundle already reached returns success without a second record.
  Refusing the retry was the alternative and is worse — a client whose
  request timed out cannot tell a refusal from a failure, which is
  exactly when it retries. Doc 14 §3 records the rule
- the racing-promotions test asserts what a user can observe: window
  ends are `1..=N` with no duplicates or gaps, window starts never move
  backwards, and no bundle is promoted twice. Asserting internal
  partition state would have passed while the observable behaviour was
  wrong
- the GC test publishes multi-megabyte chunked trees while a collector
  loops, then proves every bundle still fetches *and materializes*.
  Fetching alone would miss a collected leaf under an intact manifest
- 186 tests green

## Next Task

Batch card 18.2 (failure injection) — done, card 066.
