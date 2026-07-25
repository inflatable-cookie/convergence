# 066 Failure Injection

Status: complete
Updated: 2026-07-25
Roadmap: `g02.018`

## Objective

Break things on purpose. Every durability claim the system makes — torn
uploads heal, GC never eats a live object, corruption is caught on read,
restore and export survive a kill — was argued in a card and never
exercised.

## Scope of the actual problem

Failure paths are where the interesting bugs live and where tests are
usually absent, because writing them means building the fault. Without
the fault you are testing the claim's *statement*, not its truth.

## In Scope

- a TCP proxy that severs client→server streams mid-batch
- an object store wrapper that fails the Nth delete, so GC dies mid-sweep
- server-side object corruption, read back through a real client
- process kills during `restore` and `git export`

## Out Of Scope

- power-loss simulation below the filesystem: `write_atomic` already
  fsyncs the file and the parent directory (batch 12.4), and testing
  further needs a fault-injecting filesystem, not a test harness
- killing the *server* mid-request: its writes are one guarded batch
  (13.1), so the interesting interleavings are between requests, which
  18.1 covers

## Outcome

- **corruption was reported as 404.** A stored object failing its hash
  came back "no blobs <id>", indistinguishable from absent — while
  negotiate, which answers from a cheap existence check, kept telling
  the client the server had it. That is a loop with no exit and no
  mention of corruption. It is now a 500 naming the integrity failure;
  doc 14 §3 records the rule. Keeping `has` cheap is deliberate: hashing
  every object during negotiate would make negotiate O(bytes)
- **a killed restore left staging debris in the workspace.** The
  materialize tree was staged at `.converge-materialize-<pid>` in the
  workspace root, so a kill left it where the scan counts it as a
  pending change and the next `snap` captures it. Staging now goes
  inside `.converge`, which the scan excludes by construction
- the restore test asserts the guarantee that actually holds, not the
  one that sounds better: the swap is a per-entry delete-and-rename, so
  a kill can leave a partly-swapped tree. Real atomicity needs a journal
  or an atomic root swap, and neither is available while `.converge`
  must stay in place. What must hold — and is now tested — is that the
  store survives, a re-run completes to exactly the target tree, and
  nothing is left behind
- the upload test drives a real severed socket rather than an injected
  error, so the client sees a genuinely half-written stream; the retry
  resumes and the server's tree fetches *and* materializes
- the GC test poisons the third delete and proves the live root and its
  blobs survive, then that a clean run finishes the collection
- 191 tests green

## Next Task

Batch card 18.3 (property and pathological input) — done, card 067.
