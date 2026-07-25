# 091 Release

Status: **gated — do not start**
Updated: 2026-07-25
Roadmap: `g02.022`

## Gate

This batch does not start until the operator explicitly says so.

Their position, 2026-07-25: *"I'm not ready to release. I need to test
thoroughly locally first."* Batch 22.4 is that testing. Nothing here
runs until it is done and the operator says go.

Two independent reasons, both sufficient:

1. it publishes artifacts, which is theirs to decide
2. it touches `.github/workflows/`, which `AGENTS.md` already forbids
   without an explicit instruction

## Objective

Tagged releases with binaries people can install without a toolchain.

## Scope of the actual problem

Everything before this batch makes Convergence usable by someone who
already has it. This makes having it possible.

A release is also the first irreversible step in the roadmap. A tag can
be deleted but not un-fetched; a published binary with a defect is a
defect in someone else's hands. That asymmetry is why the shakedown
comes first — the point is to meet those defects while the only person
affected is the one who can fix them.

## In Scope

- a tag-triggered workflow building macOS and Linux binaries
- checksums, and a documented way to check them
- a one-command install that does not need Rust
- release notes generated from the batch logs, which have been written
  for a reader all along
- a documented "what to do when a release is bad"

## Out Of Scope

- package managers (Homebrew, apt): after there is something to package
- signing and notarisation: real, and their own decision

## Acceptance Criteria

- a tag produces verifiable binaries for both platforms; install works
  on a machine with no toolchain; the bad-release procedure is written
  before it is needed

## Validation

- `effigy validate`
- `effigy qa:docs`

## Next Task

None. This closes `g02.022`.
