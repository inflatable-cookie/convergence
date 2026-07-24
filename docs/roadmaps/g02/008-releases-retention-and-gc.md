# 008 Releases, Retention, and GC

Status: active
Owner: repo maintainers
Updated: 2026-07-24



## Context

`release` is the only six-verb-contract verb still missing end to end, and
GC was cut from the salvage with nothing server-side to replace it. This
roadmap completes the verb surface and makes storage honest about retention.

## Execution Plan (batch details in cards)

- **8.1 Release channels**: named channels per repo; `release` op cuts a
  bundle to a channel (policy: which gates may release); client
  `release`/`releases` verbs; TUI releases view
- **8.2 Retention policy**: per-repo/channel retention (keep-last,
  keep-days) as control-plane config; automatic-snap thinning policy from
  6.1 promoted to a shared model
- **8.3 GC**: partition-scoped mark from lane heads, window publications,
  bundles, releases per retention; object-store sweep with grace window;
  client-side GC restored for local stores; dry-run + report first
- **8.4 Determinism surfacing**: `verify` verb — re-derive a bundle from
  recorded provenance and compare hashes (the audit/compliance feature)

## Exit Criteria

- publish → promote → release → fetch-by-channel e2e; GC reclaims
  unreachable objects without touching reachable history; `verify` proves a
  bundle from provenance

## Next Task

Execute the ready Batch 8.4 card (`batch-cards/029-provenance-verify.md`).
