# Gate Graph Schema (Draft)

This document specifies the first server-side schema for gates and gate graphs.

## Goal

Represent an org-defined convergence pipeline as a validated DAG of gates, with a designated primary terminal gate.

## Gate

Fields (v1):
- `id`: stable identifier (`lowercase`, `0-9`, `-`)
- `name`: display name
- `upstream`: list of gate ids this gate consumes from
- `lane`: optional lane id that owns/operates this gate
- `policy`: promotability rules (Phase 3 minimal):
  - `allow_superpositions`: whether superpositions are allowed to pass this gate
  - `required_approvals`: number of manual approvals required to be promotable

## Gate Graph

Fields (v1):
- `version`: schema version
- `gates`: list of gate definitions
- `terminal_gate`: gate id that is the default endpoint for the primary release flow

Validation rules:
- unique gate ids
- all `upstream` references exist
- graph is acyclic
- `terminal_gate` exists
- all gates are reachable from at least one "root" gate (a gate with no upstream)

Notes:
- A repo may allow releases from non-terminal gates via policy, but `terminal_gate` remains the default.
