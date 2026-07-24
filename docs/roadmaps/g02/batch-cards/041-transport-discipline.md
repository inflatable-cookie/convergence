# 041 Transport Discipline

Status: complete
Updated: 2026-07-24
Roadmap: `g02.011`
Spec: `docs/specs/archive/011-server-trust-boundaries.md`

## Objective

Audit M2 (unbounded batch endpoints, no body limit) and the unbounded
`list_events` response closed.

## In Scope

- doc 16 §1c amendment first: server-side caps are part of the wire
  contract (max 4096 frames per batch upload, max 4096 ids per
  batch-get, 64 MiB request body limit); doc 14 §5b: event listing
  returns at most one page (1000) per call, cursor continues
- axum `DefaultBodyLimit` (64 MiB); frame-count cap on `put_batch`;
  id-count cap on `get_batch` — clear 400s naming the cap
- `list_events` LIMIT in both metadata backends
- client: `get_frames` splits requests above the id cap; `put_frames`
  flushes on frame count as well as bytes
- tests: over-cap batch-get and put_batch rejected; event listing
  bounded with cursor continuation

## Out Of Scope

- rate limiting, TLS (backlog with real identity)

## Acceptance Criteria

- oversized batch requests rejected with a clear error; large fetches
  still complete via client-side splitting; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- docs first: doc 16 §1c wire caps (4096 frames/ids, 64 MiB body),
  doc 14 §5b event paging (1000/page, cursor continues)
- server: axum `DefaultBodyLimit`, frame-count cap on batch upload,
  id-count cap on batch-get — clear 400s naming the cap; `list_events`
  LIMIT 1000 in both metadata backends
- client: `get_frames` splits requests above the id cap
  (`split_object_set`), `put_frames` flushes on frame count as well as
  bytes — large trees still sync
- tests: over-cap batch-get and upload rejected; 1005-event feed pages
  1000 then 5 via cursor — 103 workspace tests green

## Next Task

Close roadmap `g02.011`; open batch card 12.1 (safe restore) under
`g02.012`.
