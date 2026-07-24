# 2026-07-24 15:30:00 BST - Batch 6.2 Workspace Status Complete

Roadmap: `g02.006`

## Summary

One `converge status` call now answers "where am I": pending changes,
head with trigger, snap counts, remote posture. Bundle records moved to
`converge bundle <id>`; TUI root views render from the status report
alone and surface capture state.

## Validation

- `effigy validate` — 62 nextest tests green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.006` Batch 6.3 interactive-views card.
