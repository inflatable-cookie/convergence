# Batch 23.3 — Wizard Set Complete

Date: 2026-07-25
Roadmap: `g02.023`
Card: `084-wizard-set`

## What Shipped

Wizards for the four flag-heavy verbs: Member, Release, Promote, Fetch.
Each reachable by typing the bare verb; Release and Promote also from a
Bundles row (`e` and `p`), which is the screen that lists the things they
act on.

Two of the four exist to prevent a mistake rather than to save typing.
Fetch asks *where it lands* as one question with three answers, because
`--checkout` and `--into` are mutually exclusive and mean different
things — a flag list invites giving both, and a test now asserts the
wizard cannot produce the pair. Member turns a repeating `--capability`
into one field; the alternative was four yes/no questions.

Choices come from views already loaded. A wizard that blocks the event
loop on a round trip to populate a dropdown is worse than one that asks
you to type, so when the Gates or Releases view has not been opened the
field is free text.

## Four Defects, Three Of Them Older Than This Batch

### An access token was on screen and in a file

The Login wizard echoed the token while it was typed and again on the
review screen. Worse, `record_command` wrote the whole argv into the Last
strip and the agent trace wrote it to disk — a file whose own doc comment
says it keeps secrets out *because* it records argv rather than payloads.
It records argv. Argv was carrying the credential.

Batch 19.3 refused to give `secret set` a `--value` flag on precisely
this reasoning: argv lands in shell history and `ps`. `login --token` has
the same shape and shipped anyway.

Credential fields are masked in the wizard, and `redact_argv` runs
wherever argv is displayed or persisted — the same "redact where it is
formatted, so a new surface cannot forget" design as batch 19.5's output
redaction.

### `member add --issue-token` minted a token nobody could use

`shorten` truncates long space-free strings to twelve characters, which
is right for object ids and catastrophic for a token — a fresh token has
an object id's shape. The server stores only the hash, so the TUI was
handing over twelve characters of a shown-once credential with no way to
recover the rest short of revoking and reissuing.

Tokens are never shortened now. Ids still are, and the test asserts both
halves — this is not a licence to print everything.

### Wizard execution bypassed the confirmation rule

`WizardEvent::Execute` returned `Action::Run` directly, skipping
`confirmation_prompt`. Latent until this batch: no previous wizard drove
a verb on the confirm list, and Release and Promote both are.

Resolved by deciding rather than patching. The review step *is* the
confirmation — a second prompt on top would be noise — so its legend now
names the consequence, "Enter: release 01cc1008f82a", instead of saying
"run". A confirmation only counts as one if it says what is about to
happen.

### The wizard overlay never cleared what was behind it

Driving the finished Member wizard showed `Add memberde059da5
(explicit)` and `subject: danaes: 0    automatic captures: 0`. The
wizard block was compositing character-by-character over the Root view
beneath it, and had been since the wizards shipped in 17.x.

One `Clear` before the block. Reducer tests cannot catch this, because
nothing they touch draws.

## Method Note

Every defect above except the confirmation bypass was found by driving
the real binary, not by reading code. The confirmation bypass was found
by asking what the new wizards would route through — which is the
cheaper way to find things, when it works.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 268 passed, 4 skipped
- driven against a real server: member added with a usable token, release
  and fetch wizards exercised

## Next Task

Batch card 23.4 (dashboard recommendations).
