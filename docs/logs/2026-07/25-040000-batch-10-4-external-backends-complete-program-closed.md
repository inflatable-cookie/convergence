# 2026-07-25 04:00:00 BST - Batch 10.4 Complete; Improvement Program Closed

Roadmap: `g02.010`

## Summary

External backends land behind feature gates with a shared conformance
suite, closing `g02.010` — and with it the entire g02.005-g02.010
improvement program: semantics revision, continuous capture and
workspace UX, lanes and collaboration, releases/retention/GC with
provenance verify, git interop, and scale/transport. Every roadmap in
generation g02 is now closed.

The product does the full vision loop with honest storage, verified
provenance, a git adoption bridge, and a deployment story from a single
embedded binary to Postgres + S3.

## Validation

- `effigy validate` — 96 nextest tests green
- feature builds (`backend-postgres`, `backend-s3`) clippy-clean
- `effigy qa:docs` — green

## Next Task

Intent checkpoint: ask the operator for the next boundary (backlog
items or a v1.0 hardening/release boundary). No ready card exists.
