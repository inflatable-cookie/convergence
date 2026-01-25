# Bootstrapping And Identity (Operator Notes)

This document describes how to bootstrap and operate identity for the development server (`converge-server`).

## Quickstart (dev only)

If the server data dir is empty and you do not provide `--bootstrap-token`, the server auto-creates a single admin user using:
- `--dev-user` (default: `dev`)
- `--dev-token` (default: `dev`)

Example:

```bash
converge-server --addr 127.0.0.1:8080 --data-dir ./converge-data
```

Client login:

```bash
converge login --url http://127.0.0.1:8080 --repo test --token dev
```

## Recommended bootstrap flow (shared dev server)

1) Start the server with a one-time bootstrap token and an empty data dir:

```bash
BOOTSTRAP_TOKEN="$(openssl rand -hex 32)"
converge-server --addr 127.0.0.1:8080 --data-dir ./converge-data --bootstrap-token "$BOOTSTRAP_TOKEN"
```

2) Create the first admin (one-time):

```bash
curl -sS -X POST http://127.0.0.1:8080/bootstrap \
  -H "Authorization: Bearer $BOOTSTRAP_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"handle":"admin"}'
```

The response contains `token.token` (the plaintext admin token). Save it somewhere safe.

3) Restart the server without `--bootstrap-token`.

## Managing users and tokens (admin)

Assuming you have an admin token (from bootstrap or dev mode):

```bash
converge login --url http://127.0.0.1:8080 --repo test --token "$ADMIN_TOKEN"
converge whoami
```

Create users:

```bash
converge user create alice
converge user create bot-ci --admin
```

Mint a token for a user (admin):

```bash
converge token create --user alice --label "alice-laptop"
```

Users can also mint their own tokens:

```bash
converge token create --label "personal"
```

Revoke a token:

```bash
converge token revoke --id <token_id>
```

## Repo and lane membership

Grant repo access:

```bash
converge members add alice --role read
converge members add alice --role publish
converge members remove alice
```

Grant lane membership:

```bash
converge lane members default add alice
converge lane members default remove alice
```

## Notes

- If the server returns `unauthorized`, the token is missing/invalid/expired/revoked.
- The client stores remote tokens in `.converge/state.json` and does not write them to `.converge/config.json`.
- If a token is exposed, revoke it immediately and mint a new one.
