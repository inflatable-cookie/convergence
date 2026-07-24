# 2026-07-24 Batch 11.2 Complete — Namespace and Capability Integrity

Audit C3 (personal-lane squatting), arch-1.4 (missing `snap-sync`
capability), and L2 (`add_lane_member` outside `authorize`) are closed;
card 039, roadmap `g02.011`, spec 011.

## What landed

- doc 14 §4 amended first: minimal explicit capability implication
  (`snap-sync` ← `snap-sync` | `publish` | `admin`; nothing else
  implies) and the `personal/<subject>` reservation rule
- `Capability::SnapSync` in the enum; `authorize` implements the
  implication; the whole sync surface (snap upload, lane-head push,
  object writes, negotiate) regates on it — a sync-only subject can
  push unpublished lineage without holding `publish`
- `create_lane` refuses another subject's `personal/*` lane (403);
  one's own stays creatable to widen visibility
- `add_lane_member` now authorizes `read` on the repo before the
  existing owner check

## Validation

`effigy validate` green (100 tests, incl. the new
`namespace_and_capability` suite); `effigy qa:docs` green; feature
clippy clean.

## Next

Batch card 11.3: verify merges into a throwaway store; public error
messages.
