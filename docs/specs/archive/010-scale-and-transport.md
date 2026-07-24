# 010 Scale and Transport

Status: complete
Updated: 2026-07-24
Roadmap: `g02.010`

## Governing Refs

- `docs/roadmaps/g02/010-scale-and-transport.md`
- `docs/architecture/16-sync-protocol-and-chunking.md`
- `docs/architecture/14-server-authority-and-distribution.md`
- `docs/contracts/001-working-rules.md`

## Lane Focus

No semantic changes — the final program roadmap hardens representation
and transport. Determinism contracts must survive every change (the
determinism test set is the tripwire). Pre-1.0: stores re-init on the
encoding change, no shims.

## Current State

- Batch 10.1 (canonical binary encoding) complete.
- Batch 10.2 (batched transport) complete.
- Batch 10.3 (event push) complete.
- All four batches complete; roadmap `g02.010` and the improvement
  program closed.

## Exit Condition

Roadmap `g02.010` exit criteria met; the g02.005-g02.010 improvement
program closes.

## Next Task

Execute the ready Batch 10.4 card (`batch-cards/037-external-backends.md`)
— the improvement program's final batch.
