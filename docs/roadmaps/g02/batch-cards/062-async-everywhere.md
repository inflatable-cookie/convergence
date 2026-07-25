# 062 Async Everywhere

Status: complete
Updated: 2026-07-25
Roadmap: `g02.017`

## Objective

Audit P2.8, P2.9, P4.22: the TUI still ran loads on the UI thread — the
wart the UX spec calls out by name — never refreshed on its own, and had
no way to say whether the server was reachable.

## Scope of the actual problem

Three synchronous holdouts remained: the inbox load, the publish
wizard's remote probe, and `refresh` itself (status + history on every
result). The wizard probe was the clearest waste — a network round trip
to learn a gate the TUI had just been told by `status`. And with no idle
refresh, a workspace changing underneath (a `watch` in another terminal,
a teammate's publish) left a screen that looked authoritative and was
wrong.

## In Scope

- an `Intent` tag on every worker result; no argv sniffing on arrival
- inbox, resolution apply, and refresh moved to the worker; local
  commands too
- publish wizard reads its gate from the status already in hand
- idle refresh every 5s; per-view load timestamps in the header
- reachability from the event poller's own outcome

## Out Of Scope

- cancelling an in-flight command (needs a request id and a kill path
  through the CLI layer; no verb is long enough today to justify it)
- per-view refresh intervals: one idle tick is cheap because the scan
  is dirstamp-gated (batch 15.3)

## Acceptance Criteria

- no synchronous network or store load on the UI thread; views report
  their age; an unreachable server is visible; all suites green

## Outcome

- results carry an `Intent` chosen at spawn time. The old code sniffed
  argv on arrival, which had already broken down: the Bundles view and
  the inbox screen both load `inbox`, so argv could not say what the
  answer was for
- `Intent::Command` covers *every* typed command, not just remote ones.
  `snap` on a large tree and `restore` stall a frame exactly like a
  network call does, and one path is one path to get right
- the publish wizard's probe is gone rather than made async:
  `remote_gate()` parses the gate out of the status report the TUI
  already holds. The fastest network call is the one not made
- refresh returns status and history together, so one round trip feeds
  both views and the "workspace missing" probe
- a failed refresh leaves the previous screen in place. Blanking the
  view on a transient error would make the TUI less trustworthy, not
  more
- the event poller's failure *is* the reachability signal — it runs
  every 3s and its outcome is exactly "can we talk to the server". A
  failed poll updates the header and says nothing in the Last strip,
  because an unreachable server is a state, not an event
- view age is per view: entering a list view that has never loaded shows
  no age rather than inheriting the root's
- 173 tests green, including three new reducer tests

## Next Task

Batch card 17.3 (safety and flow) — done, card 063.
