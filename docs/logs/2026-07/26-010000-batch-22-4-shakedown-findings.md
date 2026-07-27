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

### 9. Ignore rules only matched the top level (fixed)

The first publish of real work was **1695 files and 33 MB** for a project
with about forty real ones. `.convergeignore` listed `target`; the rule
excluded a root build directory and let `crates/todo-core/target` — 18 MB
— straight through.

Rules were compared only against entries directly in the workspace root.
Every Rust workspace with nested crates and every JS monorepo hits that
on contact, silently.

Now a bare name matches at any depth, as `.gitignore` does, and a rule
containing a slash stays anchored to the root. Verified on the case that
found it: 4 files → 2.

**Why it survived until a real project**: there were *three* copies of
the check — `scan_memory`, `scan_store` and `dirstamp`. Two were fixed,
the tests still failed, and the third — the one `snap` actually calls —
was the one that mattered. Three copies of a rule is how a wrong rule
stays wrong; there is now one shared `is_ignored`.

### 10. A bad publish cannot be reclaimed in a single-gate repo

Cleaning up after finding #9 exercised the whole retention path, and it
does not reach.

- `converge unsnap --force` undid the local snap cleanly, refusing first
  and naming the flag — the working tree untouched, `pending` back from
  1651 to 6. The local half is fine
- server side, GC reports `287 reachable, swept 0 objects`
- `retention set --keep-bundles 2` reports "dropped 2 bundles" and still
  sweeps **zero bytes**, because publications pin the same objects
  independently. An operator reclaiming space would read that line as
  progress and get nothing
- `--keep-publication-days 0` changes nothing either. Publications drop
  only when `seq <= window_floor`, and **the window advances on
  promotion**. This repo has one entry gate, so there is nowhere to
  promote to and the floor never moves

So in a single-gate repo — the shape every new repo starts in, since
`repo create` provisions exactly one — published objects are pinned
forever. An accidental 33 MB publish is permanent.

Not fixed here: it is a design question about whether GC should be able
to reach an unpromoted window, not a bug with an obvious patch. The
throwaway deployment was rebuilt instead, which is what the situation
actually allows today.

### 11. A rebuilt server wedged every workspace that had published (fixed)

Rebuilding the deployment to reclaim finding #10's space found this
immediately:

```
error: declared base bundle c96dccd7238816e77a29173c9472ad6f4ba3d43c59aba7f8744e46701a702709 is unknown
```

The workspace still recorded the bundle it last saw from the *old*
server. Pointing at a fresh one — same URL, same repo name — it declared
a base the new server had never issued, and every publish was refused.
A dead end with no documented way out.

It sits squarely on the disaster-recovery path guide 004 §6 documents: a
restore whose bundle history differs would wedge every client that had
published before, at exactly the moment things are already going badly.

The recorded base is a claim about what *this* workspace last saw
(doc 17 §2). A server that never issued it cannot act on the claim
either way, so the honest state is "I have seen nothing" — which is what
a fresh clone declares. `publish` now clears the stale base and retries
without one, saying so:

```
note: this server does not know the bundle this workspace last saw
      (c96dccd72388); publishing without a base
published to intake: bundle c50bef0005d9 (ready to promote, 0 objects uploaded)
```

Narrow on purpose: only when a base was recorded *and* the server names
that as the reason. A base the server does know is never discarded,
because base containment is what decides supersession.

## Measurements

| | |
| --- | --- |
| Scaffold snap | 36 files, 260 KB, 0.375 s |
| First publish | 43 objects, 0.3 MiB |
| `monkey code` leaf task 001 | solved locally, 3 attempts, 77 s wall |
| Leaf-tier baseline (7B) | 11/12 local, 9 one-shot, 1 deferral |
| Repo issue, 32B, no types in context | 11 → 6 failing, then compile errors |
| Repo issue, 32B, types in context | 11 → 6 → 1 failing |
| Exemplar store | 18 → 21 verified solutions |
| Bad publish (ignores broken) | 1695 files, 33 MB |
| Same tree, ignores fixed | 44 files, 272 KB |
| Server objects, before → after rebuild | 15 MB → 436 KB |

## Next Task

Continue the shakedown. Nothing here publishes anything; `22.5` stays
gated.
