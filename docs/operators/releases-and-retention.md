# Releases And Retention (Operator Notes)

This document describes how to work with release channels and how server retention/GC treats them.

## Releases

A release is a named channel (for example `stable`, `beta`) that points at a bundle.

Releases are mutable pointers: you can create multiple releases over time in the same channel; the "latest" release for the channel is the most recently created record.

### Create a release (CLI)

Prereqs:
- You are logged in with a token that has publish permission on the repo.
- The bundle is promotable.

```bash
converge release create --channel stable --bundle-id <bundle_id>
converge release show --channel stable
converge release list
```

Notes:
- Non-admin users can only create releases from the terminal gate (per repo gate graph).

### Create a release (TUI)

- Open remote mode (Tab).
- `bundles` -> select bundle -> `release stable`
- Or use the remote shell command directly:

```text
release --channel stable --bundle-id <bundle_id>
```

### Fetch and restore a release

Fetch only (into local object store):

```bash
converge fetch --release stable
```

Fetch + restore into a directory:

```bash
converge fetch --release stable --restore --into ./out --force
```

TUI:
- `releases` -> select channel -> `fetch`
- Or: `fetch --release stable --restore --into ./out --force`

## Retention and GC

The server includes a GC endpoint:

```bash
curl -X POST -H "Authorization: Bearer <token>" \
  "http://<server>/repos/<repo_id>/gc?dry_run=false&prune_metadata=true"
```

Prune old release history (keep only the latest N releases per channel):

```bash
curl -X POST -H "Authorization: Bearer <token>" \
  "http://<server>/repos/<repo_id>/gc?dry_run=false&prune_metadata=true&prune_releases_keep_last=1"
```

This reduces retention roots: bundles referenced only by pruned releases may be deleted by the same GC run.

Retention roots (server keeps these bundles and their reachable snaps/objects):
- Pinned bundles (`pin`/`unpin`)
- Lane heads (including a small head history)
- Promotion pointers (`promotion_state` per scope)
- Releases (any bundle referenced by a release)

Important:
- If you cut a release, GC will keep the released bundle and all required objects.
- If you rely on a bundle without pinning, promoting, putting it on a lane, or releasing it, GC may eventually delete it.

## Troubleshooting

- `unauthorized`: token missing/invalid/expired/revoked; re-run `converge login ...` or mint a new token.
- `forbidden`: the token is valid but lacks required permissions; update repo membership/role.
