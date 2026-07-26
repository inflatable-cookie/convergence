# Batch 22.4 — Shakedown Findings (running)

Date: 2026-07-26 (opened)
Roadmap: `g02.022`
Card: `090-real-workspace-shakedown`

Live log. Appended to as the shakedown runs; not closed until the
operator says the run is done.

## Setup

A throwaway Tauri todo app at `~/Dev/scratch/shakedown-todo`, built with
`monkey code` doing function-scoped work under supervision, stored in
Convergence, mirrored to git.

Chosen because it is disposable, and because `monkey code` judges exactly
two languages — Rust via `rustc`, TypeScript via `tsc --noEmit` + `bun` —
which is what Tauri is made of.

Deliberate shape:

- **Monkey has its own Convergence identity**: subject `monkey`, granted
  `read` and `publish` and **not** `secret`. That is doc 19 §10a's
  agent-identity story, used rather than described
- **git mirror** via `converge git export`, so a Convergence failure
  costs the run nothing and g02.009's interop gets exercised too
- **escalation is a blocking file handshake.** `monkey code --frontier`
  runs `sh -c` with the task in `$MONKEY_REQUEST` and reads fenced code
  from stdout. Claude is not a subprocess, so the shim queues the
  request and blocks for an answer. Blocking rather than returning empty
  is deliberate: monkey only learns an exemplar when the frontier
  *returns* verified code, and that loop is the reason to pair these two

## Findings

### 1. The version stamp went stale the moment it was committed (fixed)

`converge --version` reported a commit four commits behind HEAD.

`build.rs` had `cargo:rerun-if-changed=.git/HEAD`, and on a branch
`.git/HEAD` contains `ref: refs/heads/main` — text that does not change
when you commit. Only `.git/refs/heads/main` does. So the rebuild never
triggered and the stamp silently kept naming an old commit, which is
exactly the failure the stamp exists to prevent.

Now watches the resolved ref and `packed-refs` as well. Found on the
first command of the shakedown, before a line of the app existed.

### 2. A fresh deployment silently skipped its format stamp (fixed)

Starting the server with `> ~/convergence-local/server.log` created a
file in the data directory *before* the server ran. The "is this
directory empty" freshness test said no, and the deployment went
unstamped — which "absent means 1" then makes invisible.

A stray `.DS_Store` would have done the same, which on macOS is close to
inevitable.

Freshness now means *Convergence* has not been here — no `format`, no
`meta.sqlite`, no `objects/` — rather than the directory being empty.

### 3. Workspace discovery claimed the home directory (fixed)

The worst of the three.

The personal identity directory is also called `.converge` and lives at
`~/.converge` (batch 19.1) — directly above most people's work. Discovery
walks up the tree looking for `.converge`, so running any verb where no
workspace existed matched the *identity* directory and reported:

```
ok   workspace      /Users/tom
```

`converge snap` there would have tried to capture the entire home
directory. On a real machine that is minutes-to-hours of hashing and a
snapshot of everything the user owns.

Discovery now skips a candidate that is `CONVERGE_HOME`, and requires a
`config.json` — which a workspace always has and the identity directory
never does. Both checks, because `CONVERGE_HOME` can be relocated and
the name test alone would miss it. Two regression tests: one that the
identity directory is refused, one that a real workspace below the home
directory still resolves.

### 4. Scaffolding tools delete `.converge` (recorded, not fixable)

`create-tauri-app --force` removed the workspace store while scaffolding
into the directory. Nothing Convergence can prevent, and git has the
same exposure. Worth knowing: `converge init` *after* scaffolding, not
before.

### 5. The local judge is only as good as the tests (methodology)

Not a defect — the most useful thing learned so far.

Task 001 asked for "the smallest positive integer not in `existing`".
`monkey code` solved it in 3 attempts, rustc-verified, and produced code
that returns 4 for `[2, 3]` — where the answer is 1. It passed because
the supplied tests never covered "1 is missing from a non-empty array".

In this division of labour the tests **are** the specification. A weak
test set gets confidently-verified wrong code, and the verification
makes it look settled. That is the human's half of the loop, and it is
where the attention belongs.

## Measurements

| | |
| --- | --- |
| Scaffold snap | 36 files, 260 KB, 0.375 s |
| First publish | 43 objects, 0.3 MiB |
| `monkey code` task 001 | solved locally, 3 attempts, 77 s wall |
| Exemplar store at start | 18 verified solutions |

## Next Task

Continue the shakedown. Nothing here publishes anything; `22.5` stays
gated.
