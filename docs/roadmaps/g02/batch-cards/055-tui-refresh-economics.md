# 055 TUI Refresh Economics

Status: complete
Updated: 2026-07-24
Roadmap: `g02.015`

## Objective

Audit 2.5 closed: the TUI stops rebuilding its whole client stack — and
rehashing the entire working tree — on every refresh and every keystroke
command.

## Scope of the actual problem

`converge_cli::execute` is one-shot by design: it discovers the
workspace, and for `status` it calls `current_manifest_tree`, which
reads and hashes **every file** in the workspace. That is right for a
binary that exits afterwards and wrong for the TUI, which calls it on
startup, after every command, and on every event tick. Remote verbs pay
a second tax: a fresh `reqwest` client (new connection pool) per call,
including the `events` poll every three seconds.

## In Scope

- `converge_cli::Session` + `execute_in(&session, argv)`: the same code
  path as `execute`, with per-process state the caller owns
- session caches the workspace handle (keyed by cwd), the working-tree
  scan (keyed by a dirstamp), and the remote client (keyed by base url +
  token) — all self-invalidating, no verb has to flush anything
- `Workspace::dirstamp`: metadata-only walk (name, kind, mode, size,
  mtime) over exactly the paths the manifest scan covers, ignore rules
  included, so an idle refresh stats instead of hashing
- TUI holds one session for its lifetime, shared with its worker threads
- event-driven refresh includes the inbox, not just status/history

## Out Of Scope

- caching the capture paths: `snap` and `watch` always rescan. The stamp
  cannot see a same-tick, same-size write, which is fine for a cache and
  not fine for a capture
- benchmarks (15.4)

## Acceptance Criteria

- an idle TUI refresh does no content hashing; an edited tree is picked
  up on the next refresh; one-shot CLI behaviour is unchanged; all
  suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `Session` holds three caches and `execute` became
  `execute_in(&Session::new(), ..)` — one-shot semantics are now a
  *special case* of the session path, not a separate one, so the binary
  and the TUI cannot drift
- the scan cache is keyed by `Workspace::dirstamp` rather than by an
  explicit dirty flag: a cache no verb can forget to invalidate. Same
  reasoning for the remote client, keyed by url + token so `login`
  replaces it implicitly
- the stamp's blind spot (same mtime tick, unchanged size) is recorded
  in arch doc 15 §4 and pinned by a test that deliberately rewinds an
  mtime to prove reuse is real, then moves it to prove invalidation is
  too. `snap` and `watch` never read the cache, so the blind spot cannot
  reach a capture
- `watch` keeps its own uncached rescan loop: its debounce is a capture
  correctness device, not a display refresh
- the TUI's event poller now shares the session, so the three-second
  `events` poll reuses one connection pool instead of building one per
  tick; arriving events refresh the inbox (only when it is loaded or
  on-screen) since events are exactly what changes it
- scan helpers in `manifest_scan::common` widened to
  `pub(in crate::workspace)` so the stamp walks the *same* ignore and
  sort rules as the scan — a second copy of those rules would be a
  silent drift risk
- 154 tests green

## Next Task

Batch card 15.4 (scale proof) — done, card 056.
