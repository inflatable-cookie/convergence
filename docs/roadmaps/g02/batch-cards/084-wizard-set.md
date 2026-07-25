# 084 Wizard Set

Status: complete
Updated: 2026-07-25
Roadmap: `g02.023`

## Objective

Make the flag-heavy verbs reachable without memorising their flags:
Member, Release, Promote, Fetch.

## Scope of the actual problem

The wizard deferral in spec 002 §8 named its own trigger: "observed use
where the flag surface is the obstacle". Four verbs qualify. `member add`
repeats `--capability` and carries `--scope-pattern` and
`--expires-in-days`. `fetch` has `--checkout` and `--into`, which are
mutually exclusive and mean different things (batch 16.2) — a flag list
invites giving both. `release` and `promote` each need a bundle id and a
target nobody remembers the flag name for.

Batch 23.2 constrains this before it starts: no wizard may collect a
secret, because a value must never enter the input buffer. That rules
out a sharing wizard and shapes the rest.

## In Scope

- Member, Release, Promote and Fetch wizards on the existing
  back-one-step and review pattern
- reachable from the console by a bare verb, and from Bundles rows for
  the two that act on a bundle
- choices sourced from views already loaded, never from a blocking probe

## Out Of Scope

- Bootstrap, Sync, Move/rename, Gate-graph edits: still deferred on the
  same "observed use" trigger
- anything that would collect a credential

## Acceptance Criteria

- each wizard produces the argv a person would have typed, defaults
  omitted rather than passed; the exclusive fetch flags cannot both
  appear; reducer and render tests cover it

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

Four wizards, and four defects found by building and driving them —
three of which predate this batch.

- **the Login wizard was showing an access token in the clear.** It
  echoed the token while typing and again on the review screen, and
  `record_command` wrote the whole argv into the Last strip while the
  agent trace wrote it to a file — a file whose own doc comment claims
  it keeps secrets out because it records argv rather than payloads. It
  records argv, and argv was carrying the credential. Batch 19.3 refused
  to give `secret set` a `--value` flag on exactly this reasoning;
  `login --token` had the same shape and nobody noticed. Credential
  fields are now masked, and `redact_argv` runs where argv is displayed
  or traced
- **`member add --issue-token` minted a token nobody could use.**
  `shorten` truncates long space-free strings to 12 characters for object
  ids, and a fresh token has an object id's shape. The server stores only
  the hash, so twelve characters of a shown-once credential is
  unrecoverable — you would revoke and reissue. Tokens are now never
  shortened; ids still are
- **wizard execution bypassed `confirmation_prompt`.** Latent until now:
  before this batch no wizard drove a verb on the confirm list, and
  Release and Promote both are. Resolved by deciding rather than
  patching — the review step *is* the confirmation, so its legend now
  names the consequence ("Enter: release 01cc1008f82a") instead of
  saying "run". A second prompt on top would be noise
- **the wizard overlay never cleared what was behind it.** Driving it
  showed "Add member" and a head id sharing a line, and "subject: dana"
  running into "pending changes: 0" — the two views composited
  character-by-character. Reducer tests cannot see this because nothing
  they touch draws. One `Clear` before the block
- Fetch asks *where it lands* as one question with three answers, so the
  exclusive pair cannot both be produced, and a test asserts it
- Member omits an unchanged `--scope-pattern`, so the command reads like
  the one a person would have typed
- choices come from views already loaded (`gate_names`, `channel_names`)
  and fall back to free text when they are not. A wizard that stalls the
  event loop to populate a dropdown is worse than one that asks you to
  type
- 268 tests green

## Next Task

Batch card 23.4 (dashboard recommendations).
