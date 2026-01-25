# Phase 025: Operator Docs For Releases And Retention

## Goal

Document how to operate release channels and retention/GC behavior so a dev authority can be run without reading code.

## Scope

In scope:
- Operator doc for releases (channels, creating releases, fetching/restoring).
- Operator doc for retention roots (pins, lanes, promotion pointers, releases) and how GC behaves.

Out of scope:
- Policies for pruning release history.
- Production deployment guides.

## Tasks

 - [x] Add `docs/operators/releases-and-retention.md`.
 - [x] Link operator docs from existing identity bootstrap doc.
 - [x] Ensure the doc matches the current CLI/TUI commands (`release`, `releases`, `fetch --release`, `gc`).
 - [x] Add `docs/operators/README.md` index.
 - [x] Document release pruning via GC query params.

## Exit Criteria

- A new operator can create a release, fetch it, and understand why GC keeps it.
