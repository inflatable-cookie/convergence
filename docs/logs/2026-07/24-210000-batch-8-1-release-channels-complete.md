# 2026-07-24 21:00:00 BST - Batch 8.1 Release Channels Complete

Roadmap: `g02.008`

## Summary

The six-verb contract is fully implemented: snap, publish, bundle,
promote, release, superposition all run end to end. Releases cut from
may_release gates onto named channels; channel heads advance; clients
fetch by channel. The e2e also exposed and fixed a real merge bug
(same-lane sequential publishes false-superposing under lane-keyed
supersession — now input-indexed).

## Validation

- `effigy validate` — 78 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.008` Batch 8.2 retention-policy card.
