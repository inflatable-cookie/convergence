# Northstar Instruction And Rust Audit Closeout

`g02.030` card 101 ran as one isolated worker lane: a repository-scope Rust
audit-and-repair pass through Northstar's recorder, then the target-aware
AGENTS/CLAUDE optimization.

## Rust Audit

Audit id `convergence-20260831-rust-audit`, scope `repository`, six assessed
units (the workspace root plus one per crate), 165 Rust files and ~45k lines.
Records live in Git metadata under
`.git/.../northstar/rust-quality/audits/convergence-20260831-rust-audit/`;
`report.md` and `result.json` are the finalized evidence.

Result status is `degraded`, which is the honest outcome: eight findings were
recorded and left unrepaired on purpose, and six defects sit outside the strict
projection's seven rules.

### Repaired

Nineteen files, all behaviour-preserving:

- **Doc comments reattached (15 sites, every crate).** Doc blocks had come
  adrift from the item they describe. The worst was `dispatch.rs`, where
  `run_token_prune`'s and `run_gate_change`'s docs were both stacked on
  `run_tui` while both real functions had none. `wizard.rs` carried
  `Field::new`'s doc twice, verbatim.
- **Retired terminology corrected.** `g02.028` retired channels and `g02.029`
  renamed bundle to candidate, but six CLI positions still described channels
  — including two `--release` help strings and a `doctor` line that named a
  `stable` release the code has not resolved since 28.1. `README.md`'s key
  terms still defined `bundle` two paragraphs above a note saying the term was
  renamed.
- **Two broken message strings.** `http/content.rs` and `http/gates.rs` had
  lost the `\` continuations in multi-line literals, so two user-visible
  server errors were emitted with 18-space gaps mid-sentence.
- **`impl Engine<'_> {}`** — an empty impl block left over from the engine
  module split.
- **A shadowed closure parameter** in `engine/inbox.rs`, where the closure over
  `graph.gates` bound its parameter as `candidate`, shadowing a real
  `StoredCandidate` in the same scope — at the spot whose approval logic was
  wrong in both 26.4 and 26.5.
- **Three missing `Debug` impls** in `converge-tui` (`App`, `Trace`,
  `WizardEvent`). See the review-fix wave below: `Trace` derives, while `App`,
  `Wizard` and `WizardEvent` are hand-written and redact.

### Reported, not repaired

Each is out of card 101's scope and returns to the orchestrator:

- `engine::candidate_hash` and the `publication_id` hash concatenate their
  inputs with no domain tag and no length prefixes — the collision shape
  `compute_snap_id` documents fixing in 18.3. Changing either renames every
  existing candidate.
- `cmd_run` calls `std::process::exit` inside the library path the TUI drives;
  with `CONVERGE_PASSPHRASE` set, a failing child kills the TUI without
  restoring the terminal.
- `ReleaseRequest.channel` now carries a semver version; renaming it is a wire
  break.
- `overwrite::Facts.target` is written and never read; removing it is a public
  API change.
- `gates::find_cycle` recurses over a client-supplied graph under a 64 MiB
  body limit, while its iterative sibling sits directly beneath it.
- The working-tree scan exists twice as two near-identical implementations.
- URL path segments are hand-encoded in two call sites and unencoded in five.
- `promotions.from_gate` records the producing gate rather than the gate the
  promotion actually left.

### Outside the rule catalogue

Recorded as limitations because no rule in the strict projection covers them:
the secret-file permission window in `write_atomic`, store paths built from
unvalidated ids, the machine-key generation race, `AssertGateGraph` raising a
bare error where every sibling guard raises `BatchConflict`, object writes
without `fsync` where the client fsyncs, and metadata-lock poisoning turning
one panic into permanent unavailability.

### Not found

No `unsafe` anywhere in the workspace. `RUST-ASYNC-001` passes on inspection of
all six await points: the one guard held across an await is a
`tokio::sync::Mutex`. MSRV is 1.97, declared once and inherited by all five
crates. Clippy is clean at `-D warnings` with `--all-features`.

## Review-Fix Wave

Orchestrator review of `5ee0f08` requested three changes; all are applied on
the same branch.

**Secret-bearing `Debug` surfaces.** The first wave gave `App` a redacting
`Debug` but derived one for `WizardEvent`, whose `Execute(Vec<String>)` carries
the argv the Login wizard built — including `--token <value>`. `Wizard` already
derived `Debug` while holding the same token in `values` and `input`. That was
an execution miss: `RUST-API-001`'s own carve-out is "unless doing so would
expose protected data", and it was applied to one of the three types that hold
a credential.

Both are now hand-written. `Wizard` formats its values through
`Field::display`, so a debug format obeys the same masking rule the review
screen does rather than becoming a second place that has to remember which
field is a credential. `WizardEvent::Execute` formats through
`app::redact_argv`, the helper the Last strip and the agent trace already use.

`wizard::tests::debug_output_never_carries_the_access_token` pins it at all
three moments a token exists — being typed, collected in `values`, and carried
in the emitted argv — and asserts that everything which is not a credential is
still readable. Reverting either impl to a derive fails it; that was checked,
not assumed.

**A false wire-compatibility claim.** `AGENTS.md` said "there are no pre-1.0
compatibility shims", which the code contradicts: ten `serde(alias = ...)`
reads exist across `wire.rs`, `snap.rs` and `config.rs`, and `g02.029`
documents them deliberately. The rule now states what is actually true — an
unknown major is refused outright, and an older field-name read is explicit and
is a compatibility decision rather than tidying. The same overstatement is in
`wire.rs`'s own doc comment on `WIRE_VERSION`; it is reported rather than
repaired, because the recorder finalized against that file's first-wave
content.

**Evidence scope.** The finalized recorder report covers the first wave at
`5ee0f08`. This wave's three files are not in it; they are validated by the
same merge-ready suite plus the new regression test. A second recorder run
would need its own audit id, and the orchestrator has not asked for one.

## AGENTS And CLAUDE

`AGENTS.md` had no orientation. It opened with a rule about its own leanness
and told an agent to keep six terms consistent without defining any of them —
and the audit above found terminology drift in three separate surfaces. It also
never named the contracts a change must not break.

The rewrite keeps every existing boundary and adds three sections: what
Convergence is and what each term means, what must survive a change (wire
version, on-disk format stamp, object identity, MSRV, the argv contract), and
sharp edges with their consequences. References are annotated with what each
answers. Both generated blocks are byte-identical.

`CLAUDE.md` is now exactly `@AGENTS.md`; its writing-style section duplicated
one `AGENTS.md` already owns, and no Claude-only rule was found.

Measured: 64 -> 100 non-blank lines, 839 -> 1507 approximate tokens. That is a
cost, not a score; it buys an agent the vocabulary and the invariants it
previously had to reconstruct from `docs/`.

## Validation

- Northstar recorder finalized: 6 units, 26 evidence records, all exit 0.
  That report covers the first wave only; see Review-Fix Wave above.
- `cargo nextest run -P ci -p converge-tui`: 77 passed (76 before this wave).
- `cargo nextest run -P ci`: 364 passed, 4 skipped.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- `effigy qa`, `git diff --check`, and the target-local agent-instruction audit.

Two evidence records carry `warning` status. `forwarders-converge-client` is
real: stopslop confirmed the two `*_public` forwarders already in the ledger.
`test-converge-model` is a false positive — the generic adapter matched the
substring "warning" inside the test name
`naming_the_target_yourself_is_not_a_divergence_warning`; all 42 tests passed.

## Retained Limitations

`effigy doctor` still reports 45 god-file findings and 4 attention markers.
Both predate this lane and neither was touched: the markers are false positives
on the words "review" and "stub" in ordinary comments, and the god-file list
was read as leads rather than as repair authority. The clippy threshold lints
behind it were inspected function by function and none produced a finding.

The `Effigy Agent Contract` block points at three `docs/guides/` paths that
belong to the Effigy repository and do not exist here. It sits inside generated
markers, so it was left byte-identical and is reported instead.

## Next Task

Orchestrator review of the card 101 PR head. Product execution stays paused:
`g02.027` awaits the operator's TUI cold-drive verdict and `g02.022` batch 22.5
awaits release authority.
