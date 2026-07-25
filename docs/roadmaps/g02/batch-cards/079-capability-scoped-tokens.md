# 079 Capability-Scoped Tokens

Status: complete
Updated: 2026-07-25
Roadmap: `g02.021`

## Objective

Let a token be narrower than the person holding it, so a CI job or an
agent can publish without being able to read secrets.

## Scope of the actual problem

A token today is its subject: every capability, in every repo, forever.
That makes doc 19 §10a's advice — give an agent an identity without the
`secret` capability — require creating a *second subject*, with its own
membership and its own lane. Fine for a long-lived robot, absurd for
"run this build once with a narrow credential".

The subtlety is capability implication. `authorize` treats `admin` as
satisfying everything and `publish` as satisfying `snap-sync`. A scope
that ignored implication would let a token scoped to `admin` be narrow
in name and total in effect, and a token scoped to `publish` be refused
for a snap-sync it should allow.

The second subtlety is escalation: issuing must not be a way to widen.
A scoped token issuing a broader token would make the whole mechanism
decorative.

## In Scope

- `capabilities` on the token record; empty means "whatever the subject
  has", which is the existing behaviour and stays the default
- scope enforced *before* the grant check, using the same implication
  rules `authorize` uses
- `converge token issue --capability … --label … [--expires-in-days]`,
  issuing for the calling subject
- issuance refuses any capability the caller does not effectively hold,
  so a scoped token cannot mint a wider one

## Out Of Scope

- scoping a token to a subset of *repos* or *scopes*: capability is the
  axis that matters for the agent case, and repo scoping can follow if
  something asks for it
- identity providers (21.3)

## Acceptance Criteria

- a scoped token can do exactly what its scope allows and no more, even
  when its subject is an admin; it cannot issue a wider token; an
  unscoped token behaves as before; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge token issue --capability … --label …` mints a credential for
  the caller that is narrower than they are. Doc 19 §10a's advice is now
  one command instead of a second subject with its own membership and
  lane
- **scope is checked before the grant**, and the refusal says which of
  the two it was: "this token is scoped to read, publish and does not
  carry secret" is a different problem from "you lack secret", and
  conflating them wastes somebody's afternoon
- scope uses the *same* implication rules as `authorize`, via a shared
  `satisfying_capabilities`. Without that, a token scoped to `admin`
  would be narrow in name and total in effect, and one scoped to
  `publish` would be refused for the snap-sync it should cover
- issuing cannot widen, checked twice: the caller's token must carry the
  capability, and the caller must hold it in the repo. A scoped token
  minting a broader one would make the mechanism decorative
- the test proves the interesting case — a token belonging to a **full
  admin** that cannot read a secret, while still doing everything `read`
  covers. Scope is precise rather than blunt: `secret list` needs only
  `read` (batch 19.2) and still works
- `token list` shows each token's scope, so an operator can see what
  exists without holding any of it
- 237 tests green

## Next Task

Batch card 21.3 (identity provider seam).
