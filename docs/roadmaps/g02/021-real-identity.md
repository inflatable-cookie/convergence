# 021 Real Identity

Status: complete
Owner: repo maintainers
Updated: 2026-07-25

## Context

Doc 14 §4 has said it plainly since the rebuild: authorization is fully
enforced, and *authentication* is slice-grade. Tokens map to subjects,
never expire, carry no capabilities of their own, and are revoked only
by deleting a row. Doc 14 §7 names this as the prerequisite for any
deployment beyond a trusted team.

Two batches have already built half of it without meaning to. `g02.016`
batch 16.3 issues tokens at runtime and stores them hashed; `g02.019`
batch 19.1 registers per-subject public keys. What is missing is the
part that makes a token a *statement with an expiry* rather than a
password that happens to be long.

## Findings Addressed

- tokens never expire, so a leaked one is valid until somebody notices
- revocation means deleting a database row; there is no list, no reason,
  and no audit of who revoked what
- a token carries no scope: any token is every capability its subject
  holds, in every repo
- no identity-provider story, so an organisation with SSO cannot bring
  its own accounts
- `--token subject=value` startup flags remain the documented dev path
  and have no production counterpart

## Execution Plan (batch details in cards)

- **21.1 Token lifecycle** (complete, card 078): issued-at, expires-at,
  last-used, and a revocation triple; expiry and revocation enforced at
  authentication with distinct messages; `converge token list|revoke`;
  90-day default lifetime. Found and fixed a 19.4 regression where two
  workspaces on one machine shared one credential
- **21.2 Capability-scoped tokens** (complete, card 079): `converge
  token issue --capability …`; scope checked before the grant and with
  the same implication rules `authorize` uses; issuing cannot widen. A
  token held by a full admin can publish and cannot read a secret
- **21.3 Identity provider seam** (complete, card 080): `/api/auth/config`
  and `/api/auth/exchange` verify one configured issuer's assertion and
  mint a Convergence token; `converge login --oidc` runs the device-code
  flow; static `--token` pairs stay as the offline path. Signing in
  provisions a subject with **no grants**. Testing against a fake issuer
  found three real defects: no crypto provider selected (every exchange
  would have panicked), a blocking JWKS fetch on the async worker, and
  60 seconds of inherited clock-skew leeway
- **21.4 Adversarial** (complete, card 081): a table-driven suite over
  all 41 authenticated routes. Found that scope was a property of the
  *route* rather than the token — twenty handlers called `authorize`
  directly and skipped it, so a read-scoped token could add a member
  with any capabilities, including admin. All handlers now go through
  one `authorize_scoped` entry point. Also found authentication running
  after body parsing, and moved it into a layer that runs before
  routing

## Exit Criteria

- every token has an expiry and a revocation path that does not involve
  SQL
- a token can be narrower than its subject
- an organisation can bring its own accounts without Convergence storing
  a password
- doc 14 §4's "deferred" identity note is replaced by what is built

## Outcome

All four exit criteria are met. Tokens expire and are revoked with an
audit trail; a token can be narrower than its subject; an organisation
can bring its own accounts without Convergence storing a password; and
doc 14 §4b now describes what is built in place of the deferral.

The two batches that *tested* rather than built found the most: 21.3
found three defects that would have made every exchange fail at runtime,
and 21.4 found a privilege escalation that 21.2 had shipped — scope
enforced on the routes that batch happened to touch, and bypassed on the
rest.

## Next Task

Open roadmap `g02.023` (TUI completion) with batch card 23.1.
