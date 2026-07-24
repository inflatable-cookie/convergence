# 051 Scope Registry

Status: complete
Updated: 2026-07-25
Roadmap: `g02.014`

## Objective

Audit 2.4 / M3 closed: `scope_id` is an unvalidated free string, so a
typo mints a fresh partition and silently fragments windows — work
lands in a parallel universe nobody is looking at, and no error is
raised. Scopes become declared repo state, and `scope_pattern` grants
stop lying about supporting patterns.

## Sequencing note

Batch 14.2 (async bundle builds) is **deferred, not skipped** — see the
decision recorded in the roadmap and doc 14 §7. Publish currently
commits its publication, bundle, and event in a single guarded batch
(batch 13.1); splitting the build out would reintroduce exactly the
interleaving window that batch closed, in exchange for latency relief
the beachhead has not asked for. 14.3 runs first because it closes a
live correctness gap.

## In Scope

- `scopes(repo_id, scope_id, created_at)` in both metadata backends,
  with `create_scope` / `list_scopes` / `scope_exists`; `create_repo`
  registers a `default` scope so the common path needs no ceremony
- publish, promote, release, approve, inbox, and bundle-scoped reads
  refuse an unregistered scope with an error naming the registered
  ones — before any state is touched
- `POST /api/repos/:repo/scopes` (admin) and `GET` (read); CLI verb to
  create and list
- grant patterns made true: `*`, a literal scope, or a `prefix/*`
  wildcard, matched by one shared helper used by `has_grant` in both
  backends — no other glob syntax is accepted
- tests: publish to an unregistered scope refused and nothing written;
  registered scope works; prefix grants match under the prefix and not
  outside it; conformance covers scope CRUD in both backends

## Out Of Scope

- async builds (14.2, deferred); event retention and GC scoping (14.4);
  renaming or namespacing scopes across repos

## Acceptance Criteria

- unknown scope refused with a clear, listing error and no partition
  created; `prefix/*` grants authorize exactly their subtree; all
  suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `scopes(repo_id, scope_id, created_at)` in both backends with
  `create_scope` / `list_scopes` / `scope_exists`; `create_repo`
  registers `default`
- enforcement sits in `authorize` — the one choke point every
  data-plane operation already passes through, so publish, promote,
  release, approve, inbox, and bundle reads are all covered by one
  check that runs before any state is touched. `*` stays the repo-wide
  sentinel for operations that name no single scope
- refusal names the typo and lists the registered scopes:
  `unknown scope frontnd in repo repo; registered scopes: default,
  frontend`
- `scope_pattern_matches` in `storage.rs` is the single matcher for
  both backends: `*`, literal, or `prefix/*` anchored at a path
  boundary; `foo*` is a literal, not a wildcard
- `POST`/`GET /api/repos/:repo/scopes`, client methods, and a
  `converge scope create|list` CLI verb
- eleven test harnesses now register their scopes — the churn is the
  guard proving it bites; 138 tests green
- doc 14 §1 and §4 updated to describe the registry and the real
  pattern syntax

## Next Task

Batch card 14.4 (operational hygiene).
