# Phase 032: Gate Graph Administration

Goal: make gate graph setup and evolution a first-class admin UX (not a hardcoded "dev-intake" pipeline).

Convergence depends on org-defined gates/policies. This phase adds persistence + admin APIs + CLI/TUI flows to create, validate, and update a repo's gate graph.

Non-goals:
- Fine-grained per-gate ACLs (beyond existing repo roles/admin checks).
- Multi-repo templates / org-wide inheritance.
- UI for every policy knob (start with schema v1).

## Tasks

### A) Server: Persistence + Validation

- [ ] Move `GateGraph`/`GateDef` into a shared model module (server + client types align).
- [ ] Persist gate graph per repo in the repo data file (and migrate existing repos).
- [ ] Centralize `validate_gate_graph()` (schema v1) and enforce on all writes.
- [ ] Add "reachable from a root gate" validation (currently missing).

### B) Server: Admin Endpoints

- [ ] `GET /repos/:repo_id/gate-graph` returns the current graph.
- [ ] `PUT /repos/:repo_id/gate-graph` replaces the graph (admin-only) and returns canonicalized graph.
- [ ] Return useful validation errors (cycle, unknown upstream, invalid ids, unreachable gates).
- [ ] Ensure existing operations that reference gates (publish/bundle/promote/release) validate gate existence against the stored graph.

### C) CLI: Admin UX

- [ ] `converge gate-graph show` (pretty + json).
- [ ] `converge gate-graph set --file <path>` (reads json, validates server-side).
- [ ] `converge gate-graph init` scaffolds a minimal graph (e.g. dev-intake -> terminal).

### D) TUI: Gate Graph Editor

- [ ] Add a remote admin entry point (e.g. `/gate-graph`).
- [ ] Render graph overview (gates, upstream edges, terminal gate).
- [ ] Guided flows to:
  - [ ] add/remove gate
  - [ ] edit upstream list
  - [ ] set terminal gate
  - [ ] edit policy fields (allow_superpositions, required_approvals, allow_metadata_only_publications)
- [ ] Validate locally before PUT; show server validation errors inline.

### E) Docs + Tests

- [ ] Update `docs/architecture/12-gate-graph-schema.md` if schema changes.
- [ ] Add operator docs for setting up a repo pipeline.
- [ ] Add server tests for:
  - [ ] cycle detection
  - [ ] unknown upstream
  - [ ] unreachable gates
  - [ ] terminal gate missing
  - [ ] non-admin PUT rejected

## Exit Criteria

- A server admin can set/modify a repo gate graph via CLI and TUI.
- Gate graph updates are persisted and loaded across server restart.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run -P ci` pass.
