# 091 Release

Status: in progress — pipeline built, **release not cut**
Updated: 2026-07-27
Roadmap: `g02.022`

## Gate

Partially lifted, 2026-07-27: *"You can set up some CI workflows for
22.5, then continue."*

That authorises the pipeline and the `.github/workflows/` edit. It does
not authorise cutting a release. Of the two reasons this batch was
gated —

1. it publishes artifacts, which is theirs to decide
2. it touches `.github/workflows/`, which `AGENTS.md` forbids without an
   explicit instruction

— only the second has been lifted. **No tag has been pushed and no
release exists.** The pipeline is built and proven as far as it can be
proven without publishing; the irreversible step waits for a separate
word.

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

## Progress

Built:

- `.github/workflows/release.yml` — tag-triggered, three targets on
  native runners, smoke-tests each artifact before packaging, one
  `SHA256SUMS` for the release, published with `gh` rather than a
  third-party action
- `workflow_dispatch` with `dry_run` — builds, checksums and uploads
  artifacts while creating no release, so the pipeline can be proven
  before the one step that cannot be taken back
- a `check-version` job refusing a tag that disagrees with the workspace
  version, before any platform is built
- `scripts/install.sh` — POSIX sh, curl or wget, **verifies the checksum
  before installing anything**, and `CONVERGE_BASE_URL` so the installer
  is testable against a local copy
- `docs/guides/005-releasing.md` — cutting, verifying, installing, and
  the bad-release procedure, written while calm

Proven locally, not merely written: the workflow's packaging steps were
reproduced by hand against a real build, served over HTTP, and installed
through the script. A deliberately corrupted archive was refused with
nothing left behind.

Two defects that only appeared because the artifact was handled as an
artifact:

- `git describe --tags` reached `v0-legacy`, so every build reported
  itself as `v0-legacy-108-geaf2a61` — descended from the archived g01
  tree the rebuild abandoned. Now matched to release-shaped tags only,
  falling back to a bare sha, which says less and claims nothing
- `converge-server --help` answered `unknown argument --help`. It is a
  shipped binary and that is the first thing anyone types

## Remaining

- push a tag, which is the operator's call
- confirm the three platform builds actually pass on GitHub runners; only
  the host target has been built locally
- `docs/releases/vX.Y.Z.md` for the first release body

## Validation

- `effigy validate`
- `effigy qa:docs`

## Next Task

None. This closes `g02.022`.
