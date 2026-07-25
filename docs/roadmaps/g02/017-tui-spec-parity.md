# 017 TUI Spec Parity

Status: in progress (17.1-17.3 complete)
Owner: repo maintainers
Updated: 2026-07-25

## Context

The TUI UX spec (docs/rebuild/002) is the product's UX contract; the
audit found the current TUI implements four of ten views, reintroduced
the synchronous-load wart the spec calls out by name, and leaves
promote/release/gc unconfirmed while confirming only restore. The gap
between spec and implementation must close from both ends: build the
missing surfaces, and amend the spec where the slice deliberately
simplified.

## Findings Addressed

- P2.7: Bundles, Releases, Lanes, GateGraph, Settings views missing;
  release/promote/lane/retention/gc/verify/git/annotate/init absent
  from the palette
- P1.5: TUI in an uninitialized directory is a silent dead end
- P2.8: LoadInbox, wizard remote probe, and resolution loads run
  synchronously on the UI thread (spec §7.1 wart reintroduced)
- P2.9: no idle auto-refresh or view timestamps; event refresh skips
  inbox
- P2.10: approve/promote/release/gc lack confirmation
- P2.11: resolution view lacks live validate and Alt+f/Alt+n
- P2.12: remote dashboard lacks ranked recommendations/counts;
  workflow profiles (§4.6) unimplemented
- P3.13/15: Last strip dumps truncated raw JSON; publish wizard
  hardcodes `--lane default`, making the personal-lane default
  unreachable
- P4.21: no help view; watch cannot run inside the TUI
- P4.22: no offline/reachability signal
- Minor: substring-only suggestions, missing cursor keys, trace
  missing state_change/error-class events, Tab context oddities in
  non-root views
- Test gap: zero tests for the reducer despite pure-by-design

## Execution Plan (batch details in cards)

- **17.1 Missing views** (complete, card 061): Bundles, Releases,
  Lanes, Gates and Help views, each loading through one CLI verb on the
  worker; `GET /api/repos/:repo/gates` + `converge gates` (the gate
  graph had no read path); init flow when uninitialized; full verb
  palette and Alt jump keys
- **17.2 Async everywhere** (complete, card 062): `Intent`-tagged
  worker results replace argv sniffing; inbox, resolution apply, refresh
  and every typed command run on the worker; the publish wizard's remote
  probe is gone (the gate comes from status); idle refresh every 5s;
  per-view load timestamps and a reachability signal driven by the event
  poller's own outcome
- **17.3 Safety and flow** (complete, card 063): one
  `confirmation_prompt` rule covering typed commands and inbox rows
  (approve, promote, release, restore, unsnap, `gc --execute`); pure
  live validation with `Alt+n` next missing / `Alt+f` next invalid;
  Last strip renders fields instead of truncated JSON; publish wizard's
  lane field blank by default so the server resolves the personal lane
- **17.4 Reducer test suite + spec reconciliation**: unit tests over
  `app.rs` state transitions; docs/rebuild/002 amended where the
  implementation intentionally diverges (workflow profiles decided:
  build or defer explicitly)

## Exit Criteria

- every view named in docs/rebuild/002 exists or the spec records its
  deferral
- no synchronous network or store load on the UI thread
- reducer covered by tests including confirm flows and wizard routing

## Next Task

Open batch card 17.4 (reducer suite and spec reconciliation).
