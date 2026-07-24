# 018 Gate Coalesce Strategies

Status: complete
Updated: 2026-07-24
Roadmap: `g02.005`
Spec: `docs/specs/005-convergence-semantics-revision.md`

## Objective

Implement doc 17 §4: per-gate strategy dispatch with `text-line-merge`.

## In Scope

- strategy dispatch in the fold: divergent leaf paths route to the gate's
  strategy; `whole-file` keeps current behavior
- `text-line-merge`: for divergent File/FileChunks where base and variants
  are text (no NUL in first 8 KiB, valid UTF-8): diff3 line merge of base
  vs variants folded pairwise; clean merge -> new blob (mode per doc 17);
  overlapping hunks -> superposition of the original variants; **no
  conflict markers ever written**; non-text falls back per path
- strategy + inputs already recorded in provenance (5.3); determinism
  tests extended per strategy
- tests: two lanes editing different lines of one file -> line-merged
  clean file; overlapping edits -> superposition; binary under
  text-line-merge falls back to whole-file behavior; determinism across
  fresh stores with text-line-merge

## Out Of Scope

- custom/domain strategies (later roadmap)

## Acceptance Criteria

- roadmap exit criterion holds: text edits from two lanes produce a
  line-merged file (no superposition) vs a true conflict producing one
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- semantics gap in doc 17 — revise the doc first

## Outcome

- strategy dispatch in the fold; `text-line-merge` via diff3 (`diffy`):
  clean merges become new `File` entries, overlapping hunks superpose the
  original variants, binary/non-UTF-8 falls back per path; conflict
  markers never written (asserted)
- one doc 17 amendment via stop-condition: the diff3 ancestor is the
  divergent opinions' **shared declared-base value** (W is irrelevant to
  intra-window divergence), falling back to W, then empty; opinions now
  carry their own base value to make that computable
- 4 strategy tests: disjoint-line clean merge, overlapping-conflict
  superposition, binary fallback, determinism across fresh stores;
  57 workspace tests green

## Next Task

Close roadmap `g02.005`; open `g02.006`.
