# 058 Arrival Ergonomics

Status: complete
Updated: 2026-07-25
Roadmap: `g02.016`

## Objective

Close the audit's arrival gaps: `sync pull` needed an undiscoverable
manual restore (P1.3), a bare `fetch` was invisible with no way to
continue from a bundle (P1.4), there was no read-only way to look at a
tree (P4.18), and no undo (P4.19).

## Scope of the actual problem

Every one of these is the same shape: the data arrives, and the user is
left holding an id with no verb to move forward. `pull` printed
"(restore explicitly to use it)" without naming the command; `fetch`
printed a root manifest id; History's only Enter was a destructive
restore, so looking cost you your working tree; a mistaken `snap` was
permanent.

## In Scope

- `sync pull --materialize` (with `--force`), and a named next step when
  it is not passed
- `fetch --checkout` — land the bundle in this workspace as a snap and
  continue from it; `--into` keeps meaning "write a copy elsewhere" and
  the two refuse to be combined
- `show <snap|bundle> [--path <dir>]` — record plus one directory of the
  tree, read-only
- `unsnap [--keep] [--force]` — undo the head capture
- TUI: both verbs in the palette, `unsnap` confirms once, `show` on the
  async worker

## Out Of Scope

- team onboarding (16.3), output polish (16.4)
- recursive `show` listing: one directory at a time matches the browser
  the TUI will drive, and an unbounded tree dump is the kind of
  unpaginated surface roadmap 015 spent a batch removing

## Acceptance Criteria

- pull and fetch can land work in the workspace in one command; `show`
  browses a snap or bundle without touching the tree; `unsnap` undoes a
  capture without losing content; all suites green

## Outcome

- `unsnap` undoes the *capture*, not the work: head moves to the first
  parent and the working tree is untouched, so the content returns as
  pending changes. That is why it needs no `--force` in the common case
- it refuses when the snap is not a leaf (something builds on it) and
  when it was published — `--force` overrides only the published check.
  Rewriting lineage other records point at is not undo
- the record is deleted by default. An undone capture left in the store
  reappears in history as an orphan branch, which is the opposite of
  undo; `--keep` retains it. Objects stay — they are content-addressed
  and the working tree still holds that content
- `show` takes the same ref type as `resolve` (snap id or bundle id,
  fetched on demand) by reusing `resolve_target`, so the two verbs
  cannot disagree about what an id means. Superposed paths render as
  `superposition (n variants)` rather than as files — looking at an
  unresolved tree is the main reason to look
- `fetch` now reports `{bundle_id, root_manifest, snap, materialized_to,
  next}`. A bare fetch names the two ways forward instead of printing a
  manifest id; `--checkout` uses `adopt_tree` from 16.1, so provenance
  and the head rule come for free
- `sync pull` reports `{head, materialized, next}` and names `restore
  <head>` when it did not materialize
- TUI: `unsnap` routes through the existing confirm-once path (UX spec
  §4.5); `show` joins the remote-command set since it may fetch
- 160 tests green, including an e2e that drives pull, bare fetch, show,
  and checkout against a live server

## Next Task

Batch card 16.3 (team onboarding) — done, card 059.
