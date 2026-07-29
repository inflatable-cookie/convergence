# 004 Running It Locally

Status: current
Updated: 2026-07-25
Roadmap: `g02.022` batches 22.1-22.3

How to install Convergence on your own machine, stand up a throwaway
server, and use it for real work. Nothing here publishes anything.

Doc 001 is the two-person quickstart against an already-running server;
this is the layer under it — getting the binaries and a server in the
first place, and finding out why something is not working.

## 1. Install

```sh
cargo install --path crates/converge-cli    # converge
cargo install --path crates/converge-server # converge-server
cargo install --path crates/converge-tui    # converge-tui (`converge tui` also launches it)
```

`~/.cargo/bin` needs to be on your `PATH`.

Check what you got:

```sh
converge --version
# converge 0.1.0 (v0-legacy-93-g40d945e-dirty)
```

The commit is the useful half. `0.1.0` moves rarely pre-1.0, so a note
about "0.1.0 behaviour" names a moving target; the describe string does
not. `-dirty` means the working tree had uncommitted changes when it was
built.

## 2. Stand up a server

```sh
mkdir -p ~/convergence-local
converge-server \
  --addr 127.0.0.1:2668 \
# (in this repo, `effigy server` runs exactly this against ~/convergence-local)
  --data-dir ~/convergence-local \
  --bootstrap-admin "$USER"
```

It prints one token and never prints it again — only the hash is stored.
Copy it now. Restarting with the same flag does not issue a new one; it
re-applies the grant and says so, because a restart that sprayed fresh
credentials into the logs would be worse than losing one.

Everything lives under `--data-dir`: the SQLite control plane and the
object store. That directory **is** the deployment — see §6.

## 3. First workspace

```sh
mkdir -p ~/projects/scratch && cd ~/projects/scratch
converge init
converge login --url http://127.0.0.1:2668 --token <the token> \
               --repo scratch --scope default --gate intake
converge repo create
```

`login` writes local config and contacts nobody, which is why you can
name a repo that does not exist yet and then create it.

Then work normally: `converge snap -m "..."`, `converge publish`,
`converge status`.

## 4. When something does not work

```sh
converge doctor
```

It reports workspace, personal key, remote, server reachability,
credential, access and clock skew — **all of them**, not just the first
thing that failed. Each failure names the command that fixes it.

```
ok   workspace      /Users/you/projects/scratch
FAIL personal key   no key under /Users/you/.converge
     fix: converge key init   (needed for any secret verb)
ok   remote         scratch/default/intake @ http://127.0.0.1:2668
ok   server         reachable, credential accepted
ok   clock          0s from the server
```

It also reports the store format version, which is what a "this
workspace cannot be read" message is about.

`--deep` adds a round trip that asks the server to prove it can still
serve data. Skip it day to day; run it after a restore (§6b).

It changes nothing, so it is safe to run when you are unsure. Exit
status is non-zero when something is broken, and `--json` emits one
envelope, so it works in a script:

```sh
converge --json doctor | jq -r '.data.checks[] | select(.ok == false) | .fix'
```

Two checks are worth knowing about in advance:

- **access**: the server answers "no such repo" and "you cannot read
  this repo" identically, on purpose — whether a repo exists is itself
  privileged. So the fix names both possibilities.
- **clock**: skew past 60 seconds is reported as a problem because an
  identity token that far out is refused, and the refusal blames the
  token. That sends you looking in entirely the wrong place.

## 5. Secrets, locally

```sh
converge key init          # one passphrase, no recovery
echo -n "s3cr3t" | converge secret set MY_TOKEN
converge run --secret MY_TOKEN -- ./my-script
```

`CONVERGE_PASSPHRASE` skips the prompt, which is what non-interactive
callers should use — and what the TUI needs, since a raw-mode screen has
nowhere to put a passphrase prompt (doc 19 §11).

Read doc 003 before storing anything real. The short version: there is
no recovery. Lose the passphrase and every secret sealed to that key is
gone, by design.

## 6. Backing up, restoring, and proving the restore worked

Everything the server owns is under `--data-dir`: `meta.sqlite` (the
control plane), `objects/` (every tree ever published), and `format`.
That directory **is** the deployment.

This matters more than it looks. Doc 19 §1 concedes it plainly — the
server holds secrets it cannot read, so it cannot regenerate them, and a
lost object store loses them permanently. Your backup is the only
mitigation that exists.

### Back up

Stop the server, then copy the directory:

```sh
pkill -f converge-server            # or however you run it
tar czf convergence-$(date +%F).tar.gz -C ~/convergence-local .
```

**Stop it first.** SQLite here runs in rollback-journal mode, not WAL, so
a transaction in flight leaves a `meta.sqlite-journal` beside the
database. A tar that catches one and not the other restores to a torn
state, and you find out much later.

**Back up the whole directory, not just the database.** The database
holds candidate records, releases and secret *ciphertext*; the
trees those records point at live in `objects/`. A backup of one without
the other restores a deployment that answers every question and can hand
over nothing. That is the mistake this section exists to prevent, and
§6b is how you catch it.

### What a server backup does *not* include

Personal keys. They live in `~/.converge` on each person's machine and
never reach the server — that is the entire point of doc 19's threat
model.

So there are two backups, and both are needed:

| Lost | Consequence |
| --- | --- |
| The server's data directory | Everyone loses the ciphertext. Nobody can read anything, including you. |
| Your `~/.converge` | You lose the ability to read secrets sealed to that key. The ciphertext is fine; you cannot open it. |

Neither is recoverable from the other, and neither is recoverable from
us. Back up `~/.converge` the way you would back up an SSH key.

### Restore

Put the directory back and point a server at it:

```sh
mkdir -p ~/convergence-restored
tar xzf convergence-2026-07-26.tar.gz -C ~/convergence-restored
converge-server --addr 127.0.0.1:2668 --data-dir ~/convergence-restored
```

No `--bootstrap-admin` — the users, grants and tokens came back with the
database.

### 6b. Prove the restore worked

A backup you have not restored is a hypothesis.

```sh
converge doctor --deep
```

`--deep` adds one check the ordinary run does not: it asks the server
whether it still holds the root manifest of its own `stable` release.
That is the cheapest question that touches the object store, and it is
the one that fails when a backup captured only the database:

```
FAIL serving        the server does not hold the root manifest of its own
                    stable release (9e67f65414b3)
     fix: the object store is missing or incomplete — restore from a
          backup that includes it, not just the database
```

**Run it from a workspace that has not fetched this data before.** A
`fetch` from a workspace that fetched earlier is served out of its own
local store and succeeds against a completely empty server. That is
correct behaviour and a useless test — it was observed reporting success
against a deployment with no `objects/` directory at all.

```sh
cd $(mktemp -d)
converge init
converge login --url http://127.0.0.1:2668 --token <token> --repo <repo> \
               --scope default --gate intake
converge doctor --deep
```

Then check the two things that cannot be regenerated:

```sh
converge verify <candidate-id>          # replays provenance server-side
converge secret get MY_SECRET        # decrypts with your local key
```

`verify` is the strong one: it re-runs the recorded merge on the server
and proves the candidate's identity, so it reads real objects rather than
asking whether they exist. It exits non-zero when it fails.

### What is not recoverable

- **Secrets whose key is lost.** No admin, no operator, no backup of the
  server helps. Doc 19 §5.
- **A snap that was never published.** It lives only in that workspace's
  `.converge`, which a server backup never saw.
- **A token's plaintext.** Only the hash is stored, by design. Issue a
  new one; the old record stays for the audit trail.

## 6a. If a version refuses to open your store

```
error: this workspace is format 2, and this build of Convergence reads 1.
It was written by a newer version. Upgrade Convergence, or point at a
different workspace.
Nothing has been read or written.
```

That refusal is the guard working. Nothing was read and nothing was
written, so the store is exactly as it was.

**Do not run `converge init --force`.** It refuses in this situation on
purpose, because it would discard the store rather than repair it. Get a
build that reads the format instead — `converge --version` on the one
that wrote it tells you which.

The same applies to the server and its data directory.

## 7. Throwing it away

Delete the data directory and the workspace. There is no global state
beyond `~/.converge` (personal keys and remote tokens), which you can
also delete — losing every secret sealed to those keys with it.

Set `CONVERGE_HOME` to keep an experiment's identity separate from your
real one:

```sh
CONVERGE_HOME=/tmp/throwaway-identity converge key init
```

## Next Task

Batch 22.4 is the shakedown: use this for real work and record what
breaks. Nothing in this guide publishes anything, and the release batch
does not start until you say so.
