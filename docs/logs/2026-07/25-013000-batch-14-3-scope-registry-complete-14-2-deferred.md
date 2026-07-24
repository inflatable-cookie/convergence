# 2026-07-25 Batch 14.3 Complete — Scope Registry; 14.2 Deferred

Audit 2.4 / M3 closed; card 051. Batch 14.2 (async bundle builds) is
deferred to backlog with a recorded trigger rather than built.

## The 14.2 decision

Publish commits its publication, its bundle, and its event in one
guarded batch (batch 13.1, audit H2). Making builds async splits that
into two transactions — the publication lands, the bundle follows —
which reintroduces the interleaving window batch 13.1 closed and adds a
failure mode that does not exist today (worker dies with publications
enqueued and no bundle). What it buys is publish latency, on a merge
already bounded by *changed* paths over a window promotion keeps small.

Trading a correctness property we just paid for against an unmeasured
latency improvement is a bad trade. Deferred to backlog; trigger is a
deployment showing publish latency actually hurting, at which point the
queue and its crash semantics get designed deliberately. Recorded in
doc 14 §7 and the roadmap; the roadmap's async exit criterion is struck
with its rationale, because the honesty requirement behind it is
already met by doc 14 §5 saying builds are synchronous.

## What landed for 14.3

- `scopes(repo_id, scope_id, created_at)` in both metadata backends;
  `create_scope` / `list_scopes` / `scope_exists`; `create_repo`
  registers a `default` scope so the common path needs no ceremony
- enforcement in `authorize`, the choke point every data-plane
  operation already passes — publish, promote, release, approve,
  inbox, and bundle-scoped reads are covered by one check that runs
  before any state is touched. `*` remains the repo-wide sentinel for
  operations that name no single scope
- the refusal is actionable: `unknown scope frontnd in repo repo;
  registered scopes: default, frontend`
- `scope_pattern_matches` is one shared matcher for both backends so an
  authorization decision cannot drift: `*`, a literal scope, or
  `prefix/*` anchored at a path boundary. `foo*` is a literal — the
  field no longer implies globbing it does not do
- `POST`/`GET /api/repos/:repo/scopes` (admin/read), client methods,
  `converge scope create|list`
- doc 14 §1 and §4 rewritten to describe the registry and the real
  pattern syntax

## Validation

- `effigy validate` green: 138 tests; `effigy qa:docs` green
- eleven test harnesses had to start registering their scopes — that
  churn is the guard proving it bites
- new `scope_registry` suite: unregistered publish refused with nothing
  minted, register-then-publish, pattern matching table, prefix grant
  authorizing only its subtree; conformance covers scope CRUD and
  prefix grants on both backends

## Next Task

Open batch card 14.4 (operational hygiene): event retention with
cursor-gap signalling, GC off the request thread, per-repo GC marking
where the shared store allows.
