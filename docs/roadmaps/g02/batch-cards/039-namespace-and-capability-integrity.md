# 039 Namespace and Capability Integrity

Status: complete
Updated: 2026-07-24
Roadmap: `g02.011`
Spec: `docs/specs/011-server-trust-boundaries.md`

## Objective

Audit C3 (personal-lane squatting), arch-1.4 (`snap-sync` capability
missing), and L2 (`add_lane_member` outside `authorize`) closed.

## In Scope

- `create_lane` refuses a client-supplied `lane_id` under `personal/`
  unless it is exactly the caller's own `personal/<subject>`
- `SnapSync` capability added to the enum; `authorize` treats a
  requested `snap-sync` as satisfied by a `snap-sync`, `publish`, or
  `admin` grant (publish subsumes sync; doc 14 §4 amended); snap upload
  and lane-head push regate on `SnapSync`
- `add_lane_member` authorizes `read` on the repo before the existing
  owner check
- regression tests: squatting refused (other subject's `personal/*`),
  own personal lane creatable, snap-sync-only token can push
  snaps/lane-heads but not publish, publisher token still syncs

## Out Of Scope

- real identity / token lifecycle (backlog), scope registry (14.3)

## Acceptance Criteria

- `personal/<other>` creation returns forbidden; all existing suites
  green; new capability tests green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- capability implication fights the grant model — doc 14 §4 first

## Outcome

- doc 14 §4 amended first: minimal explicit capability implication
  (`snap-sync` satisfied by `snap-sync`/`publish`/`admin`, nothing else
  implies) and the `personal/<subject>` reservation rule
- `Capability::SnapSync` added; `authorize` implements the implication;
  snap upload, lane-head push, object writes, and negotiate regate on
  `snap-sync` (a sync-only subject must upload trees; publish grants
  still satisfy via implication)
- `create_lane` refuses `personal/<other>` (403, "reserved"); one's own
  personal lane remains creatable to widen visibility
- `add_lane_member` authorizes `read` on the repo before the owner check
- new suite `namespace_and_capability.rs`: squatting refusal, own-lane
  creation + publish landing in owner's personal lane, snap-sync-only
  subject syncs but cannot publish, publish subsumes sync, no-grant
  subject denied everywhere — 100 workspace tests green

## Next Task

Batch card 11.3 (read-only means read-only).
