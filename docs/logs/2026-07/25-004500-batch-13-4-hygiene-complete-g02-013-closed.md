# 2026-07-25 Batch 13.4 Complete — Hygiene, g02.013 Closed

Audit C1, M1, C3, C2 (client), and L3 are closed; card 049. All four
batches done — roadmap `g02.013` complete.

## What landed

- **C1 validate/apply parity**: `required_superpositions` walks the
  tree the way `apply_resolution` rewrites it. A decision selecting a
  `Dir` variant descends into that subtree, so nested superpositions
  report as `missing` rather than passing validate and exploding at
  apply. Choosing a non-directory variant needs no nested decisions
- **M1 exact release deletion**: both backends match the record's
  `bundle_id` field instead of `record_json LIKE '%id%'`. Bundle ids
  sharing a prefix no longer delete each other's releases — the path
  that let GC sweep still-released objects
- **C3 recapture rules**: an explicit message on an unchanged tree now
  lands on the head record instead of being silently dropped. The card
  had proposed minting a new record; that contradicts doc 17 §1
  (identity is content + lineage, messages are editable metadata), so
  the message updates the existing record and no phantom lineage node
  appears. Dedup also covers the no-HEAD case, and `put_snap` is
  write-once with `overwrite_snap` as the explicit edit path
- **C2 locked state**: all `state.json` read-modify-writes go through
  `mutate_state`, guarded by an O_EXCL lock file with bounded wait and
  30-second stale-lock takeover. Concurrent CLI processes stop losing
  each other's updates
- **L3 canonical identity**: `compute_snap_id` length-prefixes the
  parent list and derived bundle id; domain tag is now
  `converge-snap-v3` (pre-1.0, no shim). Doc 17 §1 carries the new
  formula plus the recapture and write-once rules

## Validation

- `effigy validate` green: 134 tests; `effigy qa:docs` green
- new `hygiene` suite: nested-superposition validate/apply parity,
  recapture message persistence, no-HEAD dedup, first-writer
  preservation, eight-thread concurrent state mutation,
  parent-boundary distinctness; conformance gained exact-release-
  deletion coverage

## Roadmap g02.013 exit criteria

- concurrent publishes to one partition produce consistent
  window/base state (13.1)
- stale-bundle promote refused with a clear error (13.2)
- full merge decision table covered, including modify-vs-delete and
  nested-superposition parity (13.3, 13.4)

## Next Task

Open roadmap `g02.014` (architecture honesty), batch card 14.1.
