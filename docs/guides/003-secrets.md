# 003 Secrets

Status: active
Updated: 2026-07-25
Roadmap: `g02.019`

Storing a credential in Convergence so that only you can read it. Every
command here is exercised by `converge-cli/tests/secret_verbs.rs`.

Design and threat model: `docs/architecture/19-secrets-and-key-management.md`.

## Before anything else: agents get their own identity

If an AI agent works in your repository, the important control is not
how secrets are delivered — it is **who the agent is**. An agent running
under your credentials can run `converge secret get` exactly as you can.

```bash
converge member add my-agent --capability read --capability publish --issue-token
```

No `secret` capability, so no amount of exploring reaches a credential:
the server refuses. That is a boundary you can verify, rather than a
habit that has to hold.

## 1. Make a key

```bash
converge key init
```

Generates an X25519 keypair, seals the private half under a passphrase,
and registers the public half with the repo.

**There is no recovery.** Lose the passphrase and every secret sealed to
that key is gone — no admin and no operator can restore it. That is the
property that makes the rest of this true.

## 2. Store a secret

```bash
printf %s "$MY_PASSWORD" | converge secret set db-password
converge secret set db-password    # or type it at a hidden prompt
```

The value is read from stdin and never from a flag, because a
command-line argument lands in shell history and in every process
listing on the machine.

Secrets are sealed to *every* key you have registered, so rotating a key
never strands what you stored before it.

## 3. Use it

In order of preference:

```bash
# 1. Best: one child process, nothing at rest.
converge run --secret db-password -- ./server
converge run --secret PGPASSWORD=db-password -- psql

# 2. For an injector that already exists (a task runner, direnv, CI).
converge secret get db-password --json

# 3. Last resort, for tooling that only reads .env.
converge secret write-env .env
```

`converge run` puts named secrets in one child's environment and nothing
in yours. Its limit, stated plainly: a process environment is readable
through `/proc/<pid>/environ` by the same uid and survives into crash
dumps. It defends against discovery — grepping, wandering, an agent
reading files — and is not a wall against a determined attacker on your
machine.

`write-env` writes plaintext to disk. It warns every time, adds the path
to `.convergeignore` so no snap can capture it, and records the read.
Prefer either of the other two.

## 4. See who read what

```bash
converge events
```

`secret.read` and `secret.changed` carry the subject, the secret name,
and the version — never the value. A file on disk cannot tell you it was
read; this can, which is what turns a leaked credential into a bounded
incident.

The trade is deliberate: the server learns *when* you use each secret.
Doc 19 §10c chooses audit over read-privacy, and no setting reverses it.

## Housekeeping

```bash
converge secret list          # names, owners, versions — never values
converge secret rm NAME       # your own secrets only
converge key rotate           # new key; old one kept so nothing strands
```

Deleting a secret does not change the credential it held. If it leaked,
rotate it at its source — the AWS console, the API dashboard — and store
the new value.

## Next Task

Shared team secrets are `g02.020`: the same records with more
recipients, plus the membership and rotation questions that come with
them.
