# Phase 032: Gate Graph Administration

Goal: make gate graph setup and evolution a first-class admin UX (not a hardcoded "dev-intake" pipeline).

Convergence depends on org-defined gates/policies. This phase adds persistence + admin APIs + CLI/TUI flows to create, validate, and update a repo's gate graph.

Non-goals:
- Fine-grained per-gate ACLs (beyond existing repo roles/admin checks).
- Multi-repo templates / org-wide inheritance.
- UI for every policy knob (start with schema v1).

## Tasks

### A) Server: Persistence + Validation

- [x] Move `GateGraph`/`GateDef` into a shared model module (server + client types align).
- [x] Persist gate graph per repo in the repo data file (and migrate existing repos).
- [x] Centralize `validate_gate_graph()` (schema v1) and enforce on all writes.
- [x] Add "reachable from a root gate" validation (currently missing).

### B) Server: Admin Endpoints

- [x] `GET /repos/:repo_id/gate-graph` returns the current graph.
- [x] `PUT /repos/:repo_id/gate-graph` replaces the graph (admin-only) and returns canonicalized graph.
- [x] Return useful validation errors (cycle, unknown upstream, invalid ids, unreachable gates).
- [x] Ensure existing operations that reference gates (publish/bundle/promote/release) validate gate existence against the stored graph.

### C) CLI: Admin UX

- [x] `converge gates show` (pretty + json).
- [x] `converge gates set --file <path>` (reads json, validates server-side).
- [x] `converge gates init` scaffolds a minimal graph (e.g. dev-intake -> ship).

### D) TUI: Gate Graph Editor

- [x] Add a remote admin entry point (e.g. `/gates`).
- [x] Render graph overview (gates, upstream edges, release policy).
- [x] Guided flows to:
- [x] add/remove gate
- [x] edit upstream list
- [x] toggle release policy
- [x] edit policy fields (allow_superpositions, required_approvals, allow_metadata_only_publications)
- [ ] Validate locally before PUT; show server validation errors inline.

### E) Docs + Tests

- [x] Update `docs/architecture/12-gate-graph-schema.md` if schema changes.
- [ ] Add operator docs for setting up a repo pipeline.
- [x] Add server tests for:
  - [x] cycle detection
  - [x] unknown upstream
  - [ ] unreachable gates
  - [x] non-admin PUT rejected

## Exit Criteria

- A server admin can set/modify a repo gate graph via CLI and TUI.
- Gate graph updates are persisted and loaded across server restart.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
