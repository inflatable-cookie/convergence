# 042 - Convergence Northstar Doctrine Alignment

Status: Complete
Owner: Documentation
Created: 2026-03-06
Depends on: 041
Vision tags: `MAINT`, `UX`

## Target Envelope

| Target | Envelope | Outcome Expectation |
| --- | --- | --- |
| `MAINT` docs contract safety | `unmanaged_contract_break_count == 0` for docs section/path conventions | One canonical structure for vision, roadmaps, logs, and operational guidance |
| `UX` operator clarity | fresh agents can find the active roadmap sequence and rationale history without path ambiguity | Documentation becomes easier to navigate and maintain |

## Goal

Cut Convergence over to the Northstar documentation contract so vision, segmented roadmaps, and month-sharded logs are all explicit and consistent.

## Scope

- add the missing `vision/`, `roadmaps/`, and `logs/` spine
- move flat roadmap files into `roadmaps/g01/`
- move decision history into `logs/YYYY-MM/`
- rewrite root and docs guidance to the new contract
- remove the retired `docs/roadmap/` and `docs/decisions/` folders entirely

## Completion

- [x] Northstar core docs added
- [x] roadmap sequence moved into `docs/roadmaps/g01/`
- [x] decision history moved into `docs/logs/2026-01/`
- [x] repo guidance and internal links rewritten
- [x] old flat folders removed with no compatibility shims

## Vision Target Delta

- `MAINT`: Convergence now has one explicit docs contract instead of mixed flat folders.
- `UX`: fresh agents can find roadmap execution and rationale history without guessing between `roadmap` and `decisions`.

## Next Task

Open the next real Convergence milestone as `g01.043` and keep future execution evidence in `docs/logs/YYYY-MM/`.
