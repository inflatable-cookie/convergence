# 024 Inbox

Status: ready
Updated: 2026-07-24
Roadmap: `g02.007`
Spec: `docs/specs/007-lanes-and-collaboration.md`

## Objective

The triage surface: what needs my attention — lane activity I can see,
publications and bundles awaiting action in my scope.

## In Scope

- server `GET /api/repos/:repo/inbox?scope=<s>`: for the caller —
  readable lanes with heads newer than an optional `since` cursor,
  current-window publications for the scope's gates, non-promotable
  (superposed) ready bundles, promotable bundles short of approvals;
  one JSON report assembled engine-side with visibility respected
- client `inbox` API + CLI `converge inbox` (human + `--json`)
- TUI: inbox view (command `inbox`, Alt+i jump) listing entries with the
  recommended next action per entry (fetch / resolve / approve /
  promote); selection + Enter runs it via the console contract
- tests: inbox contents across two users (visibility filtering asserted),
  superposed bundle appears with resolve recommendation, approval-short
  bundle appears with approve recommendation

## Out Of Scope

- provenance tightening (7.4); push notifications (g02.010 events)

## Acceptance Criteria

- one call answers "what needs me"; TUI inbox actionable; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- inbox needs state the engine cannot see — extend storage deliberately

## Next Task

On completion, open the Batch 7.4 provenance-tightening card.
