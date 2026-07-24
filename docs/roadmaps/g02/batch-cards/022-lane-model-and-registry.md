# 022 Lane Model and Registry

Status: ready
Updated: 2026-07-24
Roadmap: `g02.007`
Spec: `docs/specs/007-lanes-and-collaboration.md`

## Objective

Lanes become registered server objects with ownership and ACLs; publish
provenance references registered lanes only.

## In Scope

- wire/model: `LaneRecord { lane_id, repo_id, owner, members, visibility
  (private | repo), created_at }`
- server: lane registry in the metadata store (create/get/list, member
  add); `lane` capability checks — creating needs `publish`, joining
  managed by the owner; publish rejects unregistered `lane_id`s (server
  auto-registers a personal lane per subject on first publish for the
  solo path — `personal/<subject>` owned by the subject)
- HTTP: `POST/GET /api/repos/:repo/lanes`, member management
- client + CLI: `lane create/list/join`; publish `--lane` defaults to the
  personal lane instead of "default"
- tests: registry CRUD + ACL denial, unregistered-lane publish rejection,
  personal-lane auto-registration, provenance naming registered lanes

## Out Of Scope

- unpublished sync (7.2), inbox (7.3)

## Acceptance Criteria

- publishes carry only registered lanes; personal lanes auto-provision;
  lane ACLs enforced; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- lane visibility semantics get murky — route through architecture first

## Next Task

On completion, open the Batch 7.2 unpublished-sync card.
