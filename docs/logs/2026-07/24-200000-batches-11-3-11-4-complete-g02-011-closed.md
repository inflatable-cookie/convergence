# 2026-07-24 Batches 11.3 + 11.4 Complete — g02.011 Closed

Audit H3 (verify mutates storage from a GET), L4 (error chains leak
internals), and M2 (unbounded batch endpoints) are closed; cards 040
and 041; roadmap `g02.011` server trust boundaries is **complete**.

## What landed

- `ScratchObjects` copy-on-write overlay: `verify` replays its merge
  through it, so the shared store is byte-identical before/after — a
  filesystem-snapshot regression test proves it
- error hygiene: direct store reads in handlers return a stable
  "internal error" (500) with the chain logged server-side; engine
  domain errors keep their top-level message at 400
- wire caps as contract (doc 16 §1c): 4096 frames per upload, 4096 ids
  per batch-get, 64 MiB body limit; clear 400s naming the cap; clients
  split both directions (`split_object_set`, count-aware `put_frames`)
- event listing pages at 1000 with a continuing cursor (doc 14 §5b);
  LIMIT in both metadata backends

## Roadmap outcome

All five release blockers the audit assigned to the server trust
boundary are now closed: cross-repo read disclosure (11.1), personal-
lane squatting + missing snap-sync capability (11.2), GET-that-writes
+ error leakage (11.3), unbounded transport (11.4).

## Validation

`effigy validate` green (103 tests); `effigy qa:docs` green; feature
clippy (`backend-postgres,backend-s3`) clean.

## Next

Roadmap `g02.012` data safety; batch card 12.1 (safe restore:
materialize-to-temp + swap, path-traversal validation).
