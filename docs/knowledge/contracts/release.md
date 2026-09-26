# Release

How a Convergence release is cut, verified, installed, and withdrawn.
No tagged product release has been cut yet. Workspace version is `0.1.0`.
Cutting a release is an operator action: an agent prepares it only when the
operator asks, and never pushes a tag on its own.

## What a release is

A tag `vX.Y.Z` pushed to `main`. That triggers
`.github/workflows/release.yml`, which builds three binaries —
`converge`, `converge-server`, `converge-tui` — for three targets:

| Target | Runner |
| --- | --- |
| `aarch64-apple-darwin` | macos-15 |
| `x86_64-apple-darwin` | macos-15-intel |
| `x86_64-unknown-linux-gnu` | ubuntu-22.04 |

The Linux runner is pinned older than `ubuntu-latest` on purpose: a
dynamically linked binary needs a glibc at least as new as the one it
was built against, so building on the oldest supported image is what
makes it run on the widest range of distributions.

Each is a `.tar.gz`, and the release carries a single `SHA256SUMS`
covering all of them.

Native runners rather than cross-compilation, on purpose: the workflow
runs `converge --version` on each artifact before packaging it, which is
a question only the target platform can answer.

## Before you tag

The tag must match the workspace version. `check-version` refuses the
release otherwise, before anything is built — publishing `v0.2.0` from a
tree that calls itself `0.1.0` produces binaries that misreport
themselves forever and nothing downstream can tell.

```
# 1. bump the version
$EDITOR Cargo.toml          # [workspace.package] version

# 2. prove the tree
effigy qa

# 3. optional but recommended: build the whole pipeline without
#    publishing anything
gh workflow run release.yml -f dry_run=true
```

That last step is the point of having `workflow_dispatch`. It builds,
smoke-tests, packages and checksums every target and uploads the result
as workflow artifacts — creating no release, no tag, nothing anyone can
fetch. A broken release workflow is otherwise discovered by tagging,
which is the one action here that cannot be taken back cleanly.

## Release notes

If `docs/releases/vX.Y.Z.md` exists, it becomes the release body.
Otherwise GitHub generates one from commits. Prefer the file.

## Cutting it

```
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

Then check the release page has four assets: three archives and
`SHA256SUMS`.

## Installing

User-facing install steps live in [the releasing guide](../../guides/005-releasing.md).

## When a release is bad

A tag can be deleted but not un-fetched, and a binary someone has already
installed stays installed. The order is **stop the bleeding, then tell
people, then fix**.

### 1. Stop it being installed

```
gh release edit v0.1.0 --draft
```

Drafting hides it from the releases page and from
`releases/latest/download`, which is what `install.sh` uses. New
installs stop immediately. Do this first, before diagnosing anything.

Deleting the tag as the first move breaks `git describe` for anyone who
has already fetched it and makes the bad build harder to reason about
later — and it does not un-install anything.

### 2. Say so

Edit the release body to say what is wrong, who is affected, and what to
do instead. People who already installed it will look at the release
page first.

### 3. Fix forward

Bump the patch version, cut a new release. Yanking is not a recovery: a
version that never existed is easier to reason about than one that
existed and changed.

```
$EDITOR Cargo.toml          # 0.1.0 -> 0.1.1
git tag -a v0.1.1 -m "v0.1.1"
git push origin v0.1.1
```

Never re-tag the same version onto a different commit. Anyone who
fetched the old tag keeps it, anyone who did not gets the new one, and
the two disagree permanently about what `v0.1.0` means.

### 4. Write down what got through

A release defect reached someone because the pipeline let it. Record the
failure in the owning knowledge file (what would have caught this, and
why it did not run), not as a process log.

## Deliberately not here

- **Package managers** (Homebrew, apt): after there is something worth
  packaging
- **Signing and notarisation**: real, and their own decision. Until then
  macOS Gatekeeper will quarantine a downloaded binary, and
  `xattr -d com.apple.quarantine` is the manual answer
- **Windows**: no target builds it and nothing has been tested there
