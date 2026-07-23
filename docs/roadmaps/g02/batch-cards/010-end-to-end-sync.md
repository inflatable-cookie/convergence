# 010 End-to-End Sync

Status: ready
Updated: 2026-07-23
Roadmap: `g02.003`
Spec: `docs/specs/003-rebuild-vertical-slice.md`

## Objective

Put the wire between client and server: negotiate/upload/commit over HTTP,
remote verbs in the CLI, and an end-to-end test proving the vertical slice.

## In Scope

- HTTP surface on `converge-server` (axum): token auth mapping to subjects,
  negotiate (missing objects), object upload/download, publish, approve,
  promote, bundle status; wire version checked, unknown majors refused
- minimal token issuance for the slice (server config maps token -> subject)
- client sync module in `converge-client`: manifest-walk object-set
  computation with Merkle prune, upload of missing objects, publication
  commit; fetch of a bundle root + objects
- CLI verbs: `login` (store token), `publish --gate`, `fetch <bundle>`,
  `status` (bundle status for the partition)
- e2e test: two workspaces publish divergent content → server bundle holds
  superposition → resolve locally → republish → approve → promote; dedup
  assertion (second upload negotiates to near-zero objects) and resume
  (re-run upload after partial state is idempotent)

## Out Of Scope

- TLS, real identity, external backends, edge nodes
- TUI

## Acceptance Criteria

- e2e test green against a real HTTP server on a local port
- negotiation prunes already-uploaded subtrees (asserted)
- `effigy validate` green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- wire contract needs shapes not in `converge-model::wire` — extend the DTOs
  in model first, then use them; no ad-hoc JSON

## Next Task

On completion, close roadmap `g02.003` against its exit criteria.
