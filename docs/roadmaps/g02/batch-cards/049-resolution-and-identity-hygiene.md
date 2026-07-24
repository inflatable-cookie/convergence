# 049 Resolution and Identity Hygiene

Status: complete
Updated: 2026-07-25
Roadmap: `g02.013`

## Objective

Close the roadmap's five remaining findings — the validate/apply parity
hole, the over-matching release deletion that cascades into GC, the
recapture rules that lose messages and duplicate records, the unlocked
`state.json` read-modify-write, and the latent snap-id canonicalization
weakness.

## In Scope

- **C1 recursive validation**: `validate_resolution` walks the tree the
  way `apply_resolution` rewrites it — when a decision selects a `Dir`
  variant, descend into that subtree so nested superpositions are
  reported as `missing` instead of passing validate and failing apply
- **M1 field-match release deletion**: `delete_releases_for_bundles`
  matches the record's `bundle_id` field exactly (both backends)
  instead of `record_json LIKE '%id%'`, which over-deletes releases and
  lets GC sweep still-released objects
- **C3 recapture rules**: an explicit message on an unchanged tree
  records a new snap rather than silently returning the head; dedup
  also applies when HEAD is unset (identical parentless tree returns
  the existing record); `put_snap` preserves the first writer's record
  — deliberate edits go through an explicit overwrite path
- **C2 locked state**: every `state.json` read-modify-write runs inside
  `mutate_state`, guarded by an exclusive lock file with bounded wait
  and stale-lock takeover — concurrent CLI processes no longer lose
  updates
- **L3 canonical snap identity**: `compute_snap_id` length-prefixes the
  parent list instead of joining with `,` (pre-1.0 identity change, so
  the domain tag moves to `converge-snap-v3`)
- tests: nested-superposition validate/apply parity, release deletion
  leaves same-prefix releases intact, message-bearing recapture,
  no-HEAD dedup, first-writer preservation, concurrent state mutation
  from threads, snap-id parent-boundary distinctness

## Out Of Scope

- roadmap `g02.014` architecture-honesty work; any compat shim for the
  old snap-id scheme (pre-1.0, no shims)

## Acceptance Criteria

- validate and apply agree on nested superpositions; release deletion
  is exact; recapture keeps messages and stops duplicating; concurrent
  state writers all land; snap ids distinguish parent boundaries; all
  suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- **C1**: `required_superpositions` walks the tree exactly as
  `apply_resolution` rewrites it — a decision selecting a `Dir` variant
  descends into that subtree, so nested superpositions report as
  `missing` instead of passing validate and failing apply. Choosing a
  non-directory variant needs no nested decisions
- **M1**: release deletion matches the record's `bundle_id` field in
  both backends; prefix-sharing bundle ids no longer cascade into GC
  sweeping still-released objects
- **C3**: recapture of an unchanged tree applies an explicit message to
  the head record (doc 17 §1 already makes messages editable metadata)
  rather than dropping it — the card had said "creates a record", but
  minting a lineage node for a message-only event contradicts the
  documented identity rule, so the message lands on the existing record
  instead. Dedup now also covers the no-HEAD case, and `put_snap`
  preserves the stored record with `overwrite_snap` as the explicit
  edit path
- **C2**: every `state.json` read-modify-write runs through
  `mutate_state`, guarded by an O_EXCL lock file with bounded wait and
  30s stale takeover
- **L3**: `compute_snap_id` length-prefixes parents and the derived
  bundle id; domain tag moved to `converge-snap-v3` (pre-1.0, no shim).
  Doc 17 §1 carries the new formula and the recapture/write-once rules
- tests: 134 green — new `hygiene` suite (nested-superposition parity,
  recapture message, no-HEAD dedup, first-writer preservation, 8-thread
  concurrent state mutation, parent-boundary distinctness) plus
  conformance coverage for exact release deletion

## Next Task

Close roadmap `g02.013`; open `g02.014` (architecture honesty).
