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

### 6. The build workflow had to be inverted (methodology)

`monkey code --issue` **can only rewrite functions that already exist** — it
refuses a name not found in the crate. It fixes; it does not create.

That sounds like a limitation and turns out to be the right shape. Building
an app with it means:

1. Claude writes the module skeleton — real signatures, deliberately wrong
   bodies — **and the full test suite**.
2. The suite fails. That failing suite *is* the issue.
3. Monkey localizes among the stubs and repairs until the suite is green,
   escalating when it stalls.

So the specification is executable, and finding #5 stops being a hazard and
becomes the method: the tests are the spec, and they are written first, by
the party that understands the requirement.

### 7. The judge's cost sets the architecture (measurement)

The repo path runs the crate's whole suite **on every attempt**. Putting the
pure logic in the Tauri crate would have compiled Tauri's dependency tree
each time.

So the core is a separate zero-dependency crate (`crates/todo-core`), which
the spec wanted anyway for purity. Here it is load-bearing for a different
reason: a fast judge is what makes an iterative repair loop viable at all.

### 8. The substrate had to be built before the shakedown could use it

The shakedown pairs Convergence with Monkey, whose `monkey code` does the
function-scoped work. Answering a question about which local model to use
found that **`monkey code --issue` — the repo-scale path — had no memory
at all**: it returned before the exemplar retrieval, the `--frontier`
escalation and the `learn()` call that the *same function* applies to
leaf tasks. The path doing the real work was the only one that
accumulated nothing.

Wired in Monkey as `g13.031` (escalation, learning, retrieval), then
three real defects surfaced by running it against this project:

- **warnings read as broken call sites.** `broken_call_lines` matched any
  `--> file:line:col` marker, and rustc prints those under *warnings*
  too. One `unused variable` on a stub made a cleanly-compiling crate
  look broken, so the loop chose "patch these call sites" for an issue
  where eleven tests simply failed. Four of five attempts produced output
  that could not parse
- **the model could not see the types it had to manipulate.** The loop
  retrieved the top-K *functions* and never showed the type definitions
  those functions operate on. Asked to sort by due date, a 32B wrote
  `a.due_date` where the field is `due` — every attempt, because nothing
  in the prompt said otherwise. It bites hardest in exactly the workflow
  finding #6 describes: a *stub* body has no field references to copy
  from
- two of my own, caught immediately by real input: an MSRV violation
  (Monkey holds 1.70; `is_none_or` is 1.82) and a panic slicing a string
  mid-character on an em-dash in a doc comment

With types in context the same issue went **11 → 6 → 1 failing** across
three attempts. The last failure is worth recording as a specimen: asked
for "undated tasks sort last", the model produced

```rust
.then_with(|| a.due.cmp(&b.due).then_with(|| b.due.is_some().cmp(&a.due.is_some())))
```

— the right field, the right intent, and the presence check nested
*inside* the date comparison, where it only fires on `Equal`. Since
`None.cmp(&Some(_))` is `Less`, it is dead code. Composition order, not
comprehension.

## Measurements

| | |
| --- | --- |
| Scaffold snap | 36 files, 260 KB, 0.375 s |
| First publish | 43 objects, 0.3 MiB |
| `monkey code` leaf task 001 | solved locally, 3 attempts, 77 s wall |
| Leaf-tier baseline (7B) | 11/12 local, 9 one-shot, 1 deferral |
| Repo issue, 32B, no types in context | 11 → 6 failing, then compile errors |
| Repo issue, 32B, types in context | 11 → 6 → 1 failing |
| Exemplar store | 18 → 20 verified solutions |

## Next Task

Continue the shakedown. Nothing here publishes anything; `22.5` stays
gated.
