# 005 Releasing

How a Convergence release is cut, verified, installed — and withdrawn
when it is wrong.

Written before the first release rather than after, because the part
that matters most is the last section and nobody writes that while calm.

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
effigy validate
effigy qa:docs

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
Otherwise GitHub generates one from commits.

Prefer the file. The batch logs in `docs/logs/` have been written for a
reader from the start, and a release deserves better than a list of
commit subjects.

## Cutting it

```
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

Then check the release page has four assets: three archives and
`SHA256SUMS`.

## Installing

```
curl -fsSL https://raw.githubusercontent.com/inflatable-cookie/convergence/main/scripts/install.sh | sh
```

Installs to `~/.local/bin` unless `CONVERGE_PREFIX` says otherwise, and
**verifies the checksum before installing anything** — a tampered
archive fails loudly and leaves nothing behind.

Environment:

| Variable | Meaning |
| --- | --- |
| `CONVERGE_VERSION` | A specific tag instead of the latest |
| `CONVERGE_PREFIX` | Install directory (default `~/.local/bin`) |
| `CONVERGE_REPO` | A fork |
| `CONVERGE_BASE_URL` | A mirror, an internal artifact store, or a local directory over HTTP |

`CONVERGE_BASE_URL` is also how the installer is tested without
publishing a release to find out whether it works.

### Verifying by hand

```
curl -fLO https://github.com/inflatable-cookie/convergence/releases/download/v0.1.0/SHA256SUMS
curl -fLO https://github.com/inflatable-cookie/convergence/releases/download/v0.1.0/converge-0.1.0-aarch64-apple-darwin.tar.gz
grep aarch64-apple-darwin SHA256SUMS | sha256sum -c
```

`SHA256SUMS` covers every platform, so checking the whole file fails on
the archives you did not download. Grep your line first.

### No published release yet?

```
cargo install --path crates/converge-cli
```

## After installing

```
converge doctor
```

It reports workspace, personal key, remote, server, credential, clock
and — with `--deep` — whether the server actually holds the tree it
claims to serve. Every check runs every time, so one broken thing does
not hide another.

## When a release is bad

The asymmetry to hold onto: a tag can be deleted but not un-fetched, and
a binary someone has already installed stays installed. So the order is
**stop the bleeding, then tell people, then fix**.

### 1. Stop it being installed

```
gh release edit v0.1.0 --draft
```

Drafting hides it from the releases page and from
`releases/latest/download`, which is what `install.sh` uses. New
installs stop immediately. Do this first, before diagnosing anything.

Do **not** delete the tag as the first move. Deleting it breaks
`git describe` for anyone who has already fetched it and makes the bad
build harder to reason about later — and it does not un-install
anything.

### 2. Say so

Edit the release body to say what is wrong, who is affected, and what to
do instead. People who already installed it will look at the release
page first, and it is the only channel that reaches them.

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

A release defect reached someone because the pipeline let it. Record it
where the shakedown findings live, in `docs/logs/`, with the same
question those entries ask: what would have caught this, and why did it
not run?

## Deliberately not here

- **Package managers** (Homebrew, apt): after there is something worth
  packaging
- **Signing and notarisation**: real, and their own decision. Until then
  macOS Gatekeeper will quarantine a downloaded binary, and
  `xattr -d com.apple.quarantine` is the manual answer
- **Windows**: no target builds it and nothing has been tested there

## Next Task

None. This describes a procedure; batch 22.5 owns whether it is run.
