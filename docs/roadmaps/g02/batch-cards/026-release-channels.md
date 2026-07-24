# 026 Release Channels

Status: complete
Updated: 2026-07-24
Roadmap: `g02.008`
Spec: `docs/specs/008-releases-retention-and-gc.md`

## Objective

The sixth verb: cut a bundle to a named channel under gate policy.

## In Scope

- model/wire: `ReleaseRecord { channel, repo_id, scope_id, bundle_id,
  released_by, notes, created_at }`; gate policy: `GateNode` gains
  `may_release: bool` (serde default false) — only bundles produced by a
  releasing gate can cut
- server: releases table; `release` engine op (Release capability;
  bundle ready+promotable; producing gate `may_release`); channel head =
  latest release per channel; `GET releases`, `GET release/:channel`
  (latest), `POST /api/bundles/:id/release`
- client + CLI: `release <bundle> --channel [--notes]`, `releases`,
  `fetch --release <channel>` (fetch by channel head)
- TUI: releases view (command `releases`); inbox unaffected
- tests: release policy (non-releasing gate refused, superposed refused,
  capability enforced), channel head advances, fetch-by-channel e2e

## Out Of Scope

- retention (8.2), GC (8.3), verify (8.4)

## Acceptance Criteria

- publish -> promote -> release -> fetch-by-channel e2e green; policy
  refusals tested

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- release semantics need doc 17 changes — doc first

## Outcome

- `ReleaseRecord` + releases table; engine `release` op: Release
  capability, ready+promotable bundle, producing gate must be
  `may_release` (new `GateNode` flag, serde default false)
- HTTP: release/list/channel-head routes; client + CLI `release`,
  `releases`, `fetch --release <channel>` (channel-head fetch)
- policy refusals tested: non-releasing gate, read-only subject,
  superposed bundle; channel head advances across two releases;
  fetch-by-channel materializes the released tree byte-exact
- bonus merge fix caught by the e2e: supersession was keyed on lane, so
  sequential publishes from the same (personal) lane false-superposed —
  now keyed on input index
- 78 workspace tests green

## Next Task

Execute the Batch 8.2 retention-policy card (`027-retention-policy.md`).
