# 056 Scale Proof

Status: complete
Updated: 2026-07-25
Roadmap: `g02.015`

## Objective

Close roadmap 015's exit criterion: measure publish cost on a 50k-path
tree and a large window, and pin the shape of the curve so a future
change cannot quietly reintroduce a wall.

## Scope of the actual problem

Batch 15.1 proved the merge is structurally sparse — on 5- and
50-directory trees. That is an argument, not a measurement. The audit's
claim was about real repositories, and nothing in the suite ran the fold
at a size where an accidental quadratic would show.

## In Scope

- `converge-server/tests/scale_bench.rs`, ignored by default like the
  CBOR encoding benchmark: 50k-path tree vs 5k, one-file edit; a
  100-publish window where each publish touches its own directory; a
  quiet 50-publish window
- assertions on manifest reads, timings printed as information — a
  shared CI runner cannot measure wall-clock honestly
- `effigy bench` (`cargo nextest run -P ci --run-ignored ignored-only`)
- doc 17 §2 carries the measured numbers

## Out Of Scope

- criterion or any statistical benchmark harness: the invariant is a
  read count, which is exact and needs no sampling
- incremental fold across successive publishes (still the 15.1 follow-up)

## Acceptance Criteria

- one-file edit costs the same manifest reads on 5k and 50k paths;
  window cost is flat per publish and independent of tree size; all
  suites green

## Outcome

- **The benchmark found a real wall on its first run.** Tree size was
  already clean, but a 100-publish window read **20601** manifests and
  took 20.8s: the supersession rule asks every *other* input's declared
  base for every contested path, and each ask was a fresh path walk —
  O(paths × inputs) walks, with a window's inputs usually sharing one
  base
- fixed by memoizing path walks by (root, path) for the fold's lifetime
  — objects are immutable during a merge, so the memo cannot go stale.
  100-publish window: 20601 → **801 reads**, 20.8s → 1.3s. Pure
  memoization, no semantic change, decision-table tests untouched
- measured today: one-file edit reads 9 manifests on both a 5k- and a
  50k-path tree; a 100-publish window reads 801 on both; a 50-publish
  quiet window reads 1
- the assertion is per-publish cost (`reads / window <= single-publish
  reads`), not an absolute ceiling: it states the invariant that
  actually matters and does not need retuning when the fold's constant
  changes
- `window_cost_stays_flat_per_publish` added to the always-on
  `merkle_merge` suite (16 publishes, small tree) so the quadratic
  cannot return without the ignored benchmark being run. It is the cheap
  sentinel; `scale_bench` is the measurement
- 155 default tests green; `effigy bench` runs 4 ignored ones (the three
  new scale benchmarks plus the existing CBOR encoding one)
- **CI wiring landed later**, in batch 18.4: the benchmarks run in
  `.github/workflows/nightly.yml` behind the same once-a-day,
  only-if-commits guard as the live backend lane. They were left out
  here because AGENTS.md forbids workflow edits without an explicit
  request, and the operator gave one when 18.4 built the nightly lane

## Next Task

Roadmap `g02.015` is complete. Continue the audit-hardening lane at
`g02.016` — batch card 16.1 (card 057).
