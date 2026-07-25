# 004 Running It Locally

Status: current
Updated: 2026-07-25
Roadmap: `g02.022` batch 22.1

How to install Convergence on your own machine, stand up a throwaway
server, and use it for real work. Nothing here publishes anything.

Doc 001 is the two-person quickstart against an already-running server;
this is the layer under it — getting the binaries and a server in the
first place, and finding out why something is not working.

## 1. Install

```sh
cargo install --path crates/converge-cli    # converge
cargo install --path crates/converge-server # converge-server
cargo install --path crates/converge-tui    # converge-tui
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
  --addr 127.0.0.1:8080 \
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
converge login --url http://127.0.0.1:8080 --token <the token> \
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
ok   remote         scratch/default/intake @ http://127.0.0.1:8080
ok   server         reachable, credential accepted
ok   clock          0s from the server
```

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

## 6. Backing up your local deployment

Everything the server owns is under `--data-dir`. With the server
stopped:

```sh
tar czf convergence-backup-$(date +%F).tar.gz -C ~/convergence-local .
```

Stop it first: SQLite is being written to, and a tarball of a live
database is a tarball of an unknown state.

This matters more than it looks. Doc 19 §1 concedes it plainly — the
server holds secrets it cannot read, so it cannot regenerate them, and a
lost object store loses them permanently. Your backup is the only
mitigation that exists.

Restoring is putting the directory back and starting the server against
it. **A backup you have not restored is a hypothesis**; batch 22.3 turns
this section into a tested procedure. Until then, treat it as the
minimum rather than the answer.

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

Batch 22.2 adds a store-format version, so an old workspace against a
new binary is refused with an explanation rather than misread. Batch
22.3 turns §6 into a procedure that has actually been tested.
