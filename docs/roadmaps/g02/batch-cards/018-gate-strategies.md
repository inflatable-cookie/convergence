# 018 Gate Coalesce Strategies

Status: ready
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

## Next Task

On completion, close roadmap `g02.005` against its exit criteria.
