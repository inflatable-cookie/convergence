# 045 Durability Details

Status: complete
Updated: 2026-07-24
Roadmap: `g02.012`
Spec: `docs/specs/012-data-safety.md`

## Objective

Audit R1 (`write_atomic` never fsyncs), G2 (git-map written
non-atomically, after the ref move), D3 (torn snapshots under
concurrent writes), R2 (`read_config` writes on the hot path) closed:
crash- and power-loss-safe local state, no silent torn captures.

## In Scope

- `write_atomic` fsyncs the temp file before rename and the parent
  directory after — config, state, HEAD, snaps, resolutions all become
  power-loss durable through the shared helper
- git-map saved through the same atomic + fsync'd path; export runs
  fast-import against a temporary ref, persists the updated map, and
  only then moves `refs/heads/<branch>` — a crash at any point either
  re-runs a deterministic fast-import (same shas, no duplicates) or
  finds the map complete and just moves the ref
- capture reads files stat → read → re-stat (len + mtime) with bounded
  retries; a file that keeps changing fails the snap loudly instead of
  recording a torn blob (silent small-file truncation) or a stale size
- `read_config` made pure: the legacy config-token migration (write on
  the read path) is dropped — tokens live in state.json only, pre-1.0
  posture, `converge login` recovers
- tests: kill-between-fast-import-and-map-save simulated by exporting
  twice over a torn map state produces no duplicate commits; capture of
  a file changing mid-read errors rather than snapping torn bytes

## Out Of Scope

- server-side object store fsync discipline (shares the audit's R1
  shape but its durability story is the meta/pin transaction work,
  roadmap g02.013); cross-process capture locking

## Acceptance Criteria

- kill-mid-export then re-export yields no duplicate commits; torn
  capture fails loudly; `read_config` performs no writes; all suites
  green

## Validation

- `effigy validate`

## Outcome

- `write_atomic` fsyncs the temp file before rename and the parent
  directory after — config, state, HEAD, snaps, resolutions, git-map
  all power-loss durable through the shared helper
- git export runs fast-import against `refs/converge/export-tmp`,
  persists the map atomically, then moves `refs/heads/<branch>` and
  drops the temp ref; deterministic fast-import means a crash at any
  point re-converges to identical shas (test: delete git-map after
  export, re-export — same head sha, `rev-list --count` unchanged)
- capture reads via `read_file_stable` (stat → read → re-stat, len +
  mtime, 3 attempts) in both scan paths; chunk helpers now take bytes,
  killing the second racy read; concurrent-writer test proves a
  successful snap is never torn
- `read_config` pure — legacy config-token migration dropped (pre-1.0,
  tokens live in state.json, `converge login` recovers); purity proven
  byte-for-byte under test
- 116 tests green

## Next Task

Close roadmap `g02.012`; open `g02.013` (transactional and merge
correctness).
