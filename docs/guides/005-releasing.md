# 005 Releasing

How to install a Convergence release and verify it. The operator procedure
for cutting and withdrawing a release lives in
[knowledge/contracts/release.md](../knowledge/contracts/release.md).
No tagged product release has been cut yet.

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
