# Spec 011: Server Trust Boundaries

Status: complete (archived)
Roadmap: `g02.011`
Updated: 2026-07-24

## Intent

Close the audit's release-blocking server findings (C1, C3, H3, M2, L2,
L4, arch-1.4). Every route ends up either inside `authorize()` or
behind a documented reachability check; read-only endpoints stop
mutating; request sizes are bounded.

## Execution grammar

Four batches, sequenced:

1. **11.1 Read authorization** — repo-scoped object routes with an
   object→repo association recorded on write; authorization on all
   candidate/provenance/verify reads.
2. **11.2 Namespace and capability integrity** — reserve `personal/*`,
   add `SnapSync` capability, regate snap upload + lane-head push,
   bring `add_lane_member` inside `authorize`.
3. **11.3 Read-only means read-only** — `verify` merges into a
   throwaway object store; public error messages.
4. **11.4 Transport discipline** — body limits, batch frame/byte caps,
   bounded event listing.

## Design pins

- Objects are content-addressed and deduped across repos; repo
  membership therefore lives in metadata (`object_repos` association),
  not in the object store. An object readable from repo A stays
  readable from repo B only if B also references it (both rows exist).
- Wire compatibility may break (pre-1.0): object and negotiate routes
  move under `/api/repos/:repo/`.
- Server-side merge outputs associate to the candidate's repo at write
  time.
- GC sweep removes association rows with the objects.

## Exit

Roadmap `g02.011` exit criteria; regression tests prove cross-repo
denial, squatting refusal, verify store-neutrality, and batch caps.
