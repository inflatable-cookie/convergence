# Phase 003: Gates, Bundles, and Promotion (Server-Side Convergence)

## Goal

Implement the core convergence loop on the server:
- define gates and a gate graph per repo
- accept publications as inputs to gates
- produce `bundle`s via `converge bundle` (coalesce selected inputs)
- compute promotability via gate policy
- advance bundles through gates with `converge promote`

## Scope

In scope:
- Server-side gate model:
  - gate definitions
  - gate graph (DAG) per repo
  - scope-specific state at each gate (what is current, what is promotable)
- Bundle creation:
  - select publications and/or upstream bundles as inputs
  - produce an immutable bundle object with provenance
  - allow unresolved superpositions in bundles
- Promotion:
  - enforce promotability rules
  - record provenance of promotions
- Client:
  - `converge bundle` (authorized) to request bundle creation at a gate
  - `converge promote` to promote a bundle
  - improved `converge status` to show gate/scope positions

Explicitly out of scope:
- TUI.
- Rich superposition resolution UX (beyond inspection and explicit resolution actions).
- Full policy DSL; start with a minimal built-in set of checks.

## Tasks

### A) Gate graph configuration

- [x] Define server-side gate schema (ids, names, upstream/downstream, lane ownership).
- [x] Persist and validate gate graphs (acyclic, reachable terminal, etc.).
- [x] Add APIs to list gates and graph for a repo.

Next step:
- Extend Phase 2's hard-coded `dev-intake` gate into a real per-repo gate graph configuration object.

### B) Bundle object model

- [x] Define bundle record (root manifest id, inputs, produced_by gate, scope, provenance).
- [x] Implement bundle storage and retrieval.
- [x] Implement bundle listing by `(repo, scope, gate)`.

### C) Coalescing algorithm (v1)

- [x] Define deterministic coalescing order rules.
- [x] Implement a simple coalescer:
  - [x] identical path changes coalesce
  - [x] conflicting path entries create superpositions
- [x] Store superpositions as first-class entries in manifests (no filesystem alternates).

### D) Promotability evaluation (minimal)

- [x] Define promotability record: `promotable: bool` + reasons.
- [x] Implement minimal policy checks:
  - [x] forbid promotion if unresolved superpositions exist (configurable per gate)
  - [ ] required approvals (stubbed as "manual approval recorded" initially)

### E) Promotion mechanics

- [x] Define promotion state per `(repo, scope, gate)`.
- [x] Implement `promote` API with authorization.
- [x] Ensure promotion is race-safe (per `(repo, scope, gate)` serialization).

### F) Client commands

- [x] Implement `converge bundle` client command.
- [x] Implement `converge promote` client command.
- [x] Update `converge status` to display per-gate scope state.

### G) Tests

- [x] End-to-end: publish two snaps -> bundle -> detect superposition -> resolve policy blocks promotion.
- [x] End-to-end: clean bundle -> promotable -> promote -> downstream gate state updates.

## Exit Criteria

- A repo can define a gate graph and scopes.
- Publications can be bundled at a gate into an immutable bundle.
- Bundles report promotability (true/false + reasons).
- Promotion advances a bundle to a downstream gate only when policy allows.

## Follow-on Phases

- Phase 004: TUI for inbox/superpositions/bundle promotion.
- Phase 005: Rich policy execution (CI integration) + release channels and artifacts.
