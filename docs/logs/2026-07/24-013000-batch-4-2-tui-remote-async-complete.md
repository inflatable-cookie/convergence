# 2026-07-24 01:30:00 BST - Batch 4.2 TUI Remote Views and Async Complete

Roadmap: `g02.004`

## Summary

The TUI no longer blocks on the network: remote commands run on a worker
thread with an in-flight marker, and the remote root dashboard shows the
configured target with a context-aware primary action.

## Changes

- async mpsc worker for publish/fetch/status/login; non-blocking delivery
- CLI `remote` verb; publish records last-published state
- remote root view + context-dependent primary action (login vs publish)
- reducer tests extended; 33 workspace tests

## Validation

- `effigy validate` — green
- `effigy qa:docs` — green

## Next Task

Execute the `g02.004` Batch 4.3 wizards-and-resolution card.
