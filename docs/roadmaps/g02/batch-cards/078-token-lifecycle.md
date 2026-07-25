# 078 Token Lifecycle

Status: complete
Updated: 2026-07-25
Roadmap: `g02.021`

## Objective

Give a token a beginning and an end: issued-at, expires-at, a revocation
path that is a command rather than a `DELETE`, and enough record to know
whether anyone is still using it.

## Scope of the actual problem

A token today is a password that happens to be long. It never expires,
so a leaked one is valid until somebody notices. It is revoked by
deleting a row, so there is no reason recorded, no audit, and no way to
answer "which credentials exist and who issued them".

Batch 16.3 built issuance and stored tokens hashed, which is the hard
half. What is missing is everything that makes a credential
*administrable*.

## In Scope

- token record: hash, subject, label, issued-at, issued-by, issuing
  repo, expires-at, last-used-at, revoked-at, revoked-by, reason
- expiry and revocation enforced at authentication, with distinct
  messages — "expired" and "revoked" are different problems for the
  person holding it
- `converge token list|revoke` naming tokens by a short id derived from
  the hash, so nothing has to handle a live credential to manage it
- `member add --issue-token --expires-in <duration>`, defaulting to a
  finite lifetime
- `token.issued` and `token.revoked` events

## Out Of Scope

- capability-scoped tokens (21.2): this batch is lifetime, not scope
- identity providers (21.3)
- rotating a token in place: revoke and issue is two clear operations
  and one ambiguous one fewer

## Acceptance Criteria

- an expired token is refused and says so; a revoked one is refused and
  says so; `token list` shows what exists without exposing any of it;
  the bootstrap admin path still works; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- tokens carry issued-at, issued-by, issuing repo, expiry, last-used,
  and a revocation triple (when, by whom, why). `converge token
  list|revoke` names them by a short id taken from the hash, so managing
  credentials never involves handling one
- **expired and revoked are different answers**, because they are
  different problems for the person holding it: one needs a new token,
  the other needs a conversation
- issued tokens expire in 90 days by default. `--expires-in-days 0`
  still buys a permanent one, and the listing shows "never expires"
  rather than leaving it blank — a permanent credential should be
  visible as a decision
- revoked records are kept rather than deleted. "Revoked when, by whom,
  and why" is what an incident asks, and a deleted row answers none of it
- last-used is tracked to the day. The question this field answers is
  "is anyone still using this", and a write per request to answer it
  would be a poor trade
- **a regression from 19.4 surfaced here.** Moving tokens to a shared
  home keyed by `(url, repo)` made two workspaces on one machine share
  one credential: logging in as a second person replaced the first
  person's token in *their* workspace, silently, visible later only as a
  confusing permission error. The key now includes the workspace root,
  restoring pre-19.4 scoping while keeping the credential out of the
  repository. Both key shapes are handled on migration, so an old
  workspace does not keep its plaintext
- 236 tests green

## Next Task

Batch card 21.2 (capability-scoped tokens).
