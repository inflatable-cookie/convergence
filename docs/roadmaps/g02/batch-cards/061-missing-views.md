# 061 Missing Views

Status: complete
Updated: 2026-07-25
Roadmap: `g02.017`

## Objective

Audit P2.7 and P1.5: the TUI implemented four of the spec's views, its
palette knew a third of the verbs, and starting it outside a workspace
produced an empty shell that failed every refresh in silence.

## Scope of the actual problem

The missing views are not decoration. A user could publish but never see
the gate graph they were publishing into, could release but never list
releases, and could work in lanes with no way to see them. The palette
gap is worse than it sounds: the console is the TUI's escape hatch, so a
verb missing from it was unreachable rather than merely unlisted.

## In Scope

- `GET /api/repos/:repo/gates` + `converge gates`: the gate graph had no
  read path at all
- Bundles, Releases, Lanes, Gates list views, each loading through one
  CLI verb on the worker thread
- a Help view: keys, every verb, and where this workspace points
- init flow when there is no workspace
- the full verb palette, plus Alt jump keys for the new views
- reducer tests for jumps, list bounds, view routing, and the init state

## Out Of Scope

- Settings-as-editing: the Help view shows configuration but does not
  edit it. Mutating remote config from a list view is a wizard, and
  wizards are 17.3's half of this roadmap
- per-row actions on the new views (approve from Bundles, promote from
  Gates): those need confirmations, which land in 17.3

## Acceptance Criteria

- every list view exists and loads without blocking the event loop; the
  palette covers the verb surface; an uninitialized directory offers
  `init`; all suites green

## Outcome

- `View::loader()` names the CLI verb that fills each view, so a view
  cannot show data a CLI user cannot reach. `Bundles` loads `inbox` —
  there is no bundle list endpoint by design (batch 15.2), and inventing
  one in the TUI would be exactly the divergence the argv contract
  prevents
- loads run on the worker channel, so entering a view never blocks the
  UI thread. That is part of 17.2's promise, arriving early because a
  synchronous version would have had to be written and then removed
- rows render field-driven rather than per-view formatters: these are
  CLI payloads, and a view with its own vocabulary drifts
- the init flow makes `init` the primary action whenever `status` fails,
  which is the one verb that fails exactly when there is no workspace
- `workspace_missing` overrides the context split: nothing remote is
  reachable before a workspace exists, so offering `login` there would
  be a dead end wearing a suggestion's clothes
- Help lists `COMMANDS` directly, so a verb added to the palette is
  documented by construction
- 170 tests green, including five new reducer tests

## Next Task

Batch card 17.2 (async everywhere) — done, card 062.
