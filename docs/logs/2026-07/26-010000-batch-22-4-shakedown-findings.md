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

### 12. An exemplar can carry a trick instead of a reason (Monkey)

The most interesting thing the loop has produced, and not a defect.

Issue 1 taught the store a fix for "undated tasks sort last": hoist the
presence check onto its own rung, because `Option` orders `None` first
and the rule wanted it last.

Issue 2 was the same trap in a different module. The exemplar was
retrieved (cos 0.72) and **helped** — the first attempt went from 4
failing to 1, where issue 1 had needed three attempts to get that far.

Then it misfired. The model wrote `b.at.cmp(&a.at)` — reversing the whole
comparison. That *does* push `None` last, so it satisfied the test the
exemplar was teaching, and it also reversed the times, turning
soonest-first into latest-first.

The worked example taught a **shape** ("presence is its own rung") and
the model took a **trick** ("reverse it to push `None` down"). It
generalised the outcome rather than the reasoning, and the trick passes
precisely the test that motivated the lesson.

That is worth knowing before trusting an exemplar store on real work: a
stored fix can encode a lesson that is right for its own test and wrong
next door. Issue 3 was written to test it directly — the same surface
with the trap **inverted**, where `None` must sort first — and retrieved
the same family at cos 0.78, the highest similarity yet.

### 13. A blocking escalation shim needs someone in the room

Operational, mine. The shim blocks waiting for an answer, which is what
makes monkey learn the exemplar. At a 1800-second timeout, one
escalation blocked for the full half hour against an empty room, timed
out, and **burned an escalation attempt** — the run had two and spent one
on nobody.

Now 180 seconds. Failing fast leaves the attempt available for a moment
when an answer is actually coming.

### 14. Retrieval offers confident irrelevance inside one codebase (Monkey)

Issue 4 was written in a different family — parsing, not sorting — to
test whether the store knows when to stay quiet. It does not: a
**sorting** exemplar was offered for a **parsing** task at cos 0.69,
comfortably above the 0.6 floor.

The cause is that both live in the same crate and share its vocabulary:
days, dates, records, `None`. The embedding is measuring *domain*
overlap, not task similarity.

The floor was calibrated on `code-bench`'s embedded tiers, where tasks
are independent toy problems with genuinely different vocabulary —
`gcd`, `is_palindrome`, `to_roman`. Inside one real codebase everything
shares a domain, cosine compresses, and an absolute floor stops
discriminating. A store that grows inside a single project will
increasingly offer confident, irrelevant examples.

Recorded, not fixed. The plausible answer is a domain-relative threshold
rather than an absolute one, and that needs measurement rather than a
guess.

### 15. A killed run leaves the workspace broken

`run_issue` restores every touched file on its normal exit path, which
held through a mid-request process kill earlier in this batch. It does
**not** hold through `pkill` of the run itself: the hallucinated `regex`
code stayed on disk, and the next run started from a crate that no
longer compiled — which its log dutifully reported as
`baseline: 1 compiler error(s) (does not build)`.

For a tool that rewrites your source, that is a real gap. The mitigation
available today is the one this project exists to test: **snap before
handing a tree to an editing tool.** I had not, which is why the repair
was by hand rather than `converge restore`. Now snapped before each
handoff.

### 16. The conflict cycle works end to end on real code

The first genuine superposition, from two identities rather than a
fixture. Two workspaces (`tom` as admin, `monkey` with `read, publish`
and no `secret`) checked out the same bundle and rewrote the same
function incompatibly — one to a lookup table, one to extended match
arms — each green in isolation.

Everything held: the second publish reported "ready, **blocked by
superpositions**"; the inbox named the bundle and printed the command to
run; `resolve list` found the contested path; the preview showed both
variants; `resolve apply` landed the choice as a snap and named the next
step; the republish came back "ready to promote". Publisher attribution
was right too — `monkey`'s publication is recorded against
`personal/monkey`, which I checked rather than assumed.

Worth stating because it is the design rather than a gap: resolving
**picks a variant, it does not merge**. Choosing one lane's rewrite
discarded the other's `next week`, which I then re-applied by hand. That
is conflicts-as-data working as intended, and it is a real cost a user
pays per conflict.

### 17. A variant preview spent its whole budget on the file header (fixed)

Batch 23.5's preview shows the first twelve lines of each variant. On the
two-line fixture it was built against, that was the whole file. On a real
Rust source file, **eleven of the twelve lines were the module doc
comment** — identical in both variants — and the preview truncated
exactly where the disagreement began. The feature that exists so nobody
chooses blind was showing the licence header.

The preview now drops the lines every variant shares and says how many:

```
[personal/monkey]
    … 9 line(s) identical in every variant
        const WORDS: &[(&str, i64)] = &[
[personal/tom]
    … 9 line(s) identical in every variant
        match text.as_str() {
```

Guarded both ways: it never trims when only one variant has text
(nothing to compare against), and never trims the entire preview (if the
variants agree throughout the budget, the difference is past it and the
head beats nothing).

### 18. The test suite wrote into the developer's real identity directory (fixed)

Driving the TUI against real history found this by accident. The TUI
reported `offline` and showed no recommendations, and its own agent trace
said why:

```
['inbox']  ok=false  stored token for this remote could not be decrypted
['events'] ok=false  stored token for this remote could not be decrypted
```

Chasing it turned up **493 token files in `~/.converge/tokens`**, for a
machine with a handful of workspaces. `resolve_loop_e2e.rs` and
`onboarding_e2e.rs` run `converge login` and never set `CONVERGE_HOME`,
so every suite run wrote real encrypted tokens into the developer's own
identity directory.

The exposure is worse than clutter. `machine_key()` **regenerates and
overwrites** when it cannot read the existing key, so a test run against
a home whose key is briefly unreadable would rotate it and orphan every
token the user actually depends on — silently, and unrecoverably.

Both suites now use one identity directory per test binary, in the temp
directory. Placing it *inside* each workspace was the obvious first fix
and the wrong one: the identity directory then becomes part of the tree
being captured, which broke the checkouts those tests assert on. Token
keys already include the workspace root, so one home per binary is
isolation enough.

Verified: a full `cargo nextest run --workspace` now leaves the count at
493, unchanged.

The other four CLI suites were checked and touch no identity state.

### 19. A debug build could not read a release build's token (observed)

Recorded as observed, without a confident cause, because the fix for #18
makes it moot for the suite and I did not want to guess in the log.

In `~/.converge`, with one freshly written token, the release binary
read it and a debug binary of the same commit did not — same workspace,
same home, same file, same 64-character machine key. In a *fresh* home,
debug wrote and both builds read it back. So it reproduces only against
that directory's history, which the 493 files make hard to reason about.

The plausible cause is `age`'s scrypt work factor: encryption picks one
by timing, and a release build is fast enough to choose a factor a debug
build's decryptor refuses. Worth confirming before trusting it. Practical
impact is limited to a developer running `cargo run` beside an installed
release binary — but that is exactly what happened here, so it is worth a
proper look rather than a shrug.

### 20. The TUI on real history: mostly good, one misleading empty state (fixed)

First time the TUI has seen a repo with twelve snaps, two lanes, eleven
bundles and a resolved superposition rather than a four-file fixture.

What held up: History is genuinely readable — short ids, trimmed
timestamps, messages visible at the right edge, which is exactly what
batch 23.1 fixed and this is the first data set wide enough to prove it.
Lanes lists both actors with owners. The dashboard ranks correctly and
names them: `8 publications in an open window (tom, monkey)`.

**The Bundles view said "no bundles" with eleven in the repo.** It is fed
by `inbox`, which reports only what needs *attention*, and every bundle
was ready to promote with no approvals required. The view's name promises
a list; its source is an action queue, and the empty state was the single
place that difference became visible — where it said the wrong thing.

Now it says what it lists:

```
nothing needs attention here.
this view lists bundles waiting on you — an approval, or a
superposition to resolve — not every bundle in the repo.
```

Copy split by hand because a `ListItem` does not wrap, which the first
attempt discovered by truncating at the pane edge. Empty states for
Releases, Lanes and Gates got the same treatment: `(or not loaded yet)`
was ambiguous everywhere it appeared.

### 21. `doctor --deep` passed without verifying anything (fixed)

The batch 22.3 operator guide says to prove a restore with `converge
doctor --deep`. Run against the shakedown repo — twelve snaps, eleven
bundles, two identities, a resolved conflict — it printed:

```
ok   serving        not checked: no `stable` release to ask about

nothing wrong here.
```

The one check that touches the object store asks the server for the tree
behind the `stable` release. A project in active development has not cut
a release, so the check did nothing and the report said `ok`. Every
verification the operator guide describes would have passed against a
deployment whose object store was empty.

The fallback is the bundle this workspace last saw: local, so no extra
round trip to find it, and real published history. A release is still
preferred when there is one, because it is what other people fetch.

The first fix was wrong in a quieter way. It resolved the fallback but
left five hardcoded messages saying `the stable release`, so a passing
run reported `holds the stable release's tree` for a repo with no
release, and a failing one would have sent an operator hunting for a
release that never existed. The subject is now threaded through every
message.

Against the restored deployment:

```
ok   serving        holds the tree of the last bundle this workspace saw (cb59de7525b6)
```

### 22. The CLI printed ids its own commands rejected (fixed)

`converge fetch` reports success as `fetched bundle cb59de7525b6`. Paste
that back:

```
$ converge verify cb59de7525b6
error: server returned 404 Not Found: {"error":"no bundle cb59de7525b6"}
```

`bundle` and `show` refused it too. The tell was in the success output
that produced the id: the `next:` hint beside it spells out all
sixty-four characters, because whoever wrote the hint already knew short
ids did not work.

Resolution now happens in the metadata store, so every route that takes
a bundle id accepts the printed form rather than each handler needing to
remember. Full ids skip the lookup. Eight characters is the floor.
Ambiguity is an error and never a guess — silently picking one of two
candidates would approve or promote the wrong bundle. The hex check is
load-bearing: it keeps `LIKE` wildcards out of the pattern.

Covered in `backend_conformance`, so both backends answer the same way.

### 23. The restore drill, against history worth keeping

The 22.3 procedure had only ever run against a synthetic fixture. Re-run
against the live deployment — 904 KB, twelve snaps, two identities, a
resolved superposition, two encrypted secrets — it holds:

- backup 257 KB, restored into a fresh directory, served at the live address
- `doctor --deep` green, and now verifying something real (finding 21)
- `converge verify` replayed the merge and reproduced the recorded bundle
- `converge fetch` pulled the tree back
- `converge secret get` decrypted, so sealed values survive a restore

The one thing the drill could not check was which credentials in
`~/.converge/tokens` were live: 493 files, nearly all debris from
finding 18's test-suite bug, and nothing distinguished them. Finding 25.

### 24. `converge run` delivers secrets and nothing else leaks

Two secrets set, then a build probe run under `converge run --secret`:
present in the child, inherited by its own children as any environment
variable is, and absent from the parent shell afterwards. Doc 19 §10
describes this exactly.

### 25. A cached login outlived its workspace, unaccountably (fixed)

493 files under `~/.converge/tokens`, and no way to tell the one live
credential from 492 dead ones.

The cause is in the naming. A token file is
`blake3(url#repo#workspace_root).age` — hashed so a directory listing
does not enumerate which servers this machine talks to, which is worth
keeping — and it held the bare token and nothing else. Delete a
workspace and its credential is orphaned: nothing removes it, and
nothing can even say which workspace it was for. Every temporary test
workspace left one behind.

The file now holds the key alongside the token, inside the encrypted
body. The listing stays as opaque as it was; staleness becomes decidable,
because the workspace either exists or it does not. Reading a legacy file
rewrites it in the new shape, so ordinary use migrates the store and what
stays unattributable is precisely what nothing has opened.

`converge token prune` reports by default and deletes only with
`--execute`, following `gc`. It sits under `token` because that is where
someone will look, though it is the only verb there that needs neither a
workspace nor a server — which is exactly the situation it exists for.

The dry-run default paid for itself within a minute. `root_dir()` is the
`.converge` directory, not the workspace above it, so the first
staleness test looked for `.converge/.converge/config.json`, found
nothing, and classified the one live credential on this machine as dead.
With `--execute` wired straight through, that would have been a working
login deleted during its own verification.

Unattributable files need `--forget-unattributable` on top, because
removing one costs a re-login and no evidence says it is dead — only
that nothing has used it lately.

On this machine, after migrating the live workspace by using it: 1 live,
492 unattributable, 1 stale from a workspace deleted as a drill. Swept to
1. The live credential still authenticates.

Four tests cover it, including one that forges a legacy file with the
machine key so the migration path is exercised rather than assumed.

### 26. The release cycle on real history, and half of finding 22 still live

A release cycle the repo had never run: `release cb59de7525b6 --channel
stable`, then consume it from a genuinely clean workspace with a
read-scoped token — the 22.3 lesson being that a workspace which has
fetched before can fake success from its own local store. 44 files
landed, byte-identical to the source tree apart from `.git`, `.DS_Store`
and one file newer than the release. `doctor --deep` now prefers the
release over the last-seen bundle, as intended.

The release verb took a shortened id, because finding 22 fixed the
server. `converge show <12-char snap id>` still answered "neither a
local snap nor a reachable bundle" — the local lookup is a filename, and
a prefix is not one. Resolved the same way, in `get_snap`, so every
caller benefits: exact ids skip the directory read, ambiguity names the
candidates rather than guessing, because `restore` and `unsnap` take
these and the wrong one is somebody's work.

Noted and not fixed: `history` prints full 64-character snap ids while
the TUI and message text shorten to twelve. Both are now accepted
everywhere, so this is width, not a dead end.

### 27. Every aborted publish leaked storage that GC could never reclaim (fixed)

The real deployment held 109 objects. GC marked 108 reachable and swept
none, on every run, for as long as the deployment existed.

The odd object was pinned. Pins exist because batch 12.2 fixed the
opposite bug — GC sweeping an upload that had not been published yet —
and the sweep comment says so plainly: pins "are the real protection for
not-yet-referenced objects", with the mtime grace covering only the
sub-millisecond gap between writing the object and writing its pin.

A pin is released by walking the tree of a *publication*. If the publish
never happens — an abort, a crash, a killed run, all of which this
shakedown produced — nothing ever releases it. The table had no
timestamp at all, so nothing could tell a three-second-old upload from a
three-month-old abandoned one. The fix for over-collection had become a
permanent leak.

`object_pins` gained `pinned_at`, and the pin predicate takes a cutoff.
Asking at the predicate rather than deleting first is what lets a dry run
and a real run reach the same answer without the dry run mutating
anything; clearing expired rows is pure tidying and is skipped on a dry
run. The grace is a day, against an upload-to-publish gap measured in
seconds — deliberately loose, because expiring early costs a failed
publish and expiring late costs a day of disk.

Existing deployments migrate on open, their rows defaulting to the epoch
and so stale at once. That is the right answer for them: they are
precisely the abandoned pins. The narrow risk is an upload in flight
across the upgrade, which a restart would have failed anyway.

On the live deployment: 9 pins, 109 objects → 0 pins, 108 objects. The
release still replays from provenance and still fetches cold into a
clean workspace.

### 28. `watch` behaves, and history would not tell you it had run (fixed)

Driven against a real editing session: one edit captured after the quiet
period, five edits a second apart coalesced into a single snap, an idle
tree captured nothing, and killing the process left no debris and no
pending changes. The debounce is a two-tick stability check, which is
the right shape.

What is wrong is downstream. An automatic snap carries no message, and
`history` printed only id, timestamp and the message — so its row was an
id and a date and nothing else. `status` says `(automatic)`, the record
says `trigger: automatic`, the `--json` history carries it; only the one
view whose job is listing snaps dropped it. After an afternoon of
`watch`, most rows look identical and none say why. Now labelled
`(automatic)`; an explicit snap with no message still shows nothing,
because that was somebody's choice.

Two things checked rather than assumed, both fine. Thinning already
spares the head and explicit snaps, keeps everything under an hour, and
`lineage_walk_tolerates_thinned_ancestors` covers the dangling-parent
case. And thinning a *published* automatic snap is harmless:
`last_published` is only ever compared, never dereferenced, and the
server holds its own copy of the record.

### 29. The git mirror attributed every commit to nobody (fixed)

`converge git export` mirrored 14 snaps to a branch whose tree is
byte-identical to the workspace, incrementally, with the
`Converge-Snap:` trailer that makes the next export incremental. All
correct.

Every commit was authored `Converge <converge@local>` — in a repo with
two identities in it. On a branch that exists to be read with git tools,
that leaves `git log --author`, `git blame` and forge attribution
showing one placeholder for all of history.

The mirror is a git artifact, so it now takes git's own identity from
`user.name` / `user.email`, falling back to the old placeholder when
those are unset, and rejecting values containing a newline or an angle
bracket because they land in a fast-import command.

The limit is worth stating: this attributes the whole lineage to whoever
runs the export. A local snap record carries no author at all — identity
is attached at publish, server-side — so per-snap attribution is not
available to ask for. Right for one person's workspace, wrong for a
history that mixes authors, and fixing it properly means putting an
author on the snap record. Logged, not done.

### 30. `sync pull --materialize` silently discarded diverged work (fixed)

The multi-person path, driven properly for the first time: a second
person added through `member add --issue-token`, their own workspace
seeded from the `stable` release, both editing the same file.

What works. The lane was created on first `sync push` without being
declared, and pushed 5 objects rather than the tree. A private personal
lane refused a pull from `tom` — who is an *admin* — with a clear
message, which is the same rule secrets use: admin subsumes capabilities
but not recipiency. After `lane add-member`, the pull fetched into the
local store and pointedly did not touch the working tree, offering
`--materialize` or `restore` as the next step.

Then `sync pull --lane personal/alex --materialize`, from a head that
had diverged. It replaced the working tree, moved head, and said
`pulled lane head 4752f9e11920 (workspace updated)`. The other person's
committed snap was simply gone from the tree. Nothing warned, nothing
asked, nothing mentioned that the snap record survives and a `restore`
brings it back.

`--force` already existed on this verb — for *pending changes*.
Divergence is the other way to lose work and it was unguarded, which is
the more dangerous of the two: pending changes are visible in `status`,
while a diverged head looks exactly like an up-to-date one.

`head_left_behind_by` returns the current head when it is not an
ancestor of the target. The refusal names both snaps, states that the
record is kept, gives the `restore` that brings it back, and gives the
`--force` that proceeds. A missing ancestor record reads as "not an
ancestor": snaps get thinned, and the cautious reading of an incomplete
lineage is the safe one. Fast-forward and same-snap cases still proceed,
which the test pins as explicitly as the refusal.

### 31. Your own newest work sat mid-list, unmarked (fixed)

Straight after that pull, `converge history` — whose help said "List
snaps, newest first" — showed:

```
4752f9e1…  18:13:28  alex: urgency decays when ignored
0d56f900…  18:13:17  checkout of bundle cb59de7525b6
b4891414…  18:13:40  tom: urgency climbs while waiting
```

The newest snap is third. The listing is ordered by lineage from head,
then everything unreachable, and `list_snaps` documents exactly that —
so the code was right and the help text was wrong.

Sorting by time would be the wrong fix; lineage order is more useful,
and it is the *reason* the row is where it is. What was missing is that
nothing said so. Rows off head's lineage now read `[off your current
line]`, and the help says what the order actually is. The moment this
matters is precisely the moment above: your work has just been moved
aside, and the list gives you no way to see which entry is yours.

### 32. The departure path holds; what it tells you to type does not (fixed)

The whole secrets-and-membership sequence on the real repo, with the
second person on their own identity home — the first attempt shared
`~/.converge` between both, which would have let "alex" decrypt with
tom's key and proved nothing. Worth recording as a trap for anyone
testing this: on one machine the crypto boundary is only real if the
homes are separate.

What holds, all of it:

- a secret sealed to alex but with no `secret` capability is refused by
  the server — recipiency and capability are separate gates, and both bite
- `member remove` lists every secret still sealed to the leaver, avoids
  the word revoke, and says they keep what they already read
- the removed member's next `secret get` is refused
- rotating without unsharing first warns that the value is being re-sealed
  to someone who left — the 20.4 trap, firing correctly on real data
- `secret unshare` then clears it, `audit` goes quiet, and the next
  rotation warns about nothing

What does not hold is the copy-and-paste. `member remove` printed the
list of secrets still sealed to alex and then, directly underneath,
`converge secret unshare <name> --from alex`. The rotation warning named
the secret and the person in its first sentence, then said
`converge secret unshare <name> --from <subject>` in its second. Both
now print one runnable line per secret. This product has spent whole
batches making its output paste-able — inbox recommendations, doctor
fixes, `member add`'s "they run:" — and these two were the exception.

`secret audit` also reported `stale recipient alex` twice, because
staleness is recorded per registered key and alex had two. One person
with a laptop and a desktop is one stale recipient. Deduplicated on the
rendered line, so reasons that are genuinely key-specific still show
separately — they name the key.

### 33. The gate graph cannot be changed after the repo is created

`converge repo create` provisions one `intake` gate. There is no verb to
add a second, no verb to edit approvals or `may_release`, and no write
route on the server either — `/api/repos/:repo/gates` is `get` only, and
`set_gate_graph` is called exactly once, at repo creation.

So `promote` — one of the six verbs the contract is built around — cannot
be reached by any real user. The refusal is correct and clear:

```
$ converge promote cb59de7525b6 --to intake
gate intake does not accept promotions from intake
```

but no downstream gate can ever exist to name instead. Everything the
multi-gate design describes — staged review, required approvals, a
release-only final gate — is implemented server-side and unreachable.

Not fixed here, deliberately. A write path is not the hard part; what
happens to *in-flight state* is. Partition state is keyed by gate, bundle
windows advance per gate, and publications sit in a gate's open window —
so removing or re-parenting a gate with live bundles is a data question
before it is an API question. That needs a card, not an improvisation
at the end of a shakedown.

### 34. Retention could wedge a gate permanently (fixed)

Two ordinary commands, on a real repo:

```
converge retention set --keep-bundles 5
converge gc --execute
```

After that, every publish failed, identically, forever:

```
published to intake: bundle 34a5651587f2 (failed: no bundle c50bef0005d9, 6 objects uploaded)
```

A publication records the base it was written against, and folding the
window re-reads that base every time. GC protected bundles named by a
surviving *release*, and bundles named as base by another *bundle* — but
not bundles named as base by a *publication*. Publication 2 declared
`c50bef00`; retention dropped it; the fold could no longer complete.

Three things made it terminal rather than annoying:

- publications only leave a window when it advances, a window only
  advances on promotion, and a single-gate repo cannot promote — finding
  33, arriving as a consequence rather than a curiosity
- the client re-derives its base and retries, so it never stops asking
- GC's publication-dropping loop iterates *partitions*, and a partition
  row is only written once a window advances — so the repo this happened
  to had twelve publications and no partition rows at all

There was no way back through the CLI. Repairing the live deployment
meant editing `meta.sqlite` by hand to null the dangling base references,
which is not a thing a user can be asked to do.

GC now protects every base declared by a surviving publication,
enumerated over scopes and gates rather than partitions, for the reason
above. The regression test publishes six times, sets `keep_bundles: 2`,
runs GC, and publishes again — and it fails without the fix, which was
worth checking: the first version of it passed both ways, because each
bundle happened to record the same base its publication had declared,
and the existing bundle-base protection covered it.

The output deserves a note too: `published to intake: bundle X (failed:
...)` reads as a contradiction. It is accurate — the publication landed,
the bundle build failed — but it announces the id of a bundle that does
not work, and leads with the success.

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
| Issue 1 (11 failing) | 11 → 6 → 1 local, then 1 escalation |
| Issue 2 (4 failing), exemplar retrieved | **4 → 1 in one attempt**, then 1 escalation |
| Exemplar store | 18 → 22 verified solutions |
| Live deployment | 904 KB, 12 snaps, 11 bundles, 2 identities |
| Backup archive | 257 KB |
| Restore → verified replay | bundle reproduced from provenance |
| Stale tokens in `~/.converge` | 493 → 1 |
| Release consumed cold, read-only token | 44 files, byte-identical |
| Server objects / pins before → after | 109 / 9 → 108 / 0 |
| Watch: 5 edits 1s apart | 1 snap |
| Git mirror | 14 commits, tree byte-identical |
| Lane push, changed file | 5 objects |
| Diverged materialize | head replaced silently, 0 warnings |
| `retention set` + `gc --execute` | gate wedged permanently, 6 publications orphaned |

## Next Task

Continue the shakedown. Nothing here publishes anything; `22.5` stays
gated.
