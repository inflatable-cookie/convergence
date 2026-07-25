# 021 Real Identity

Status: in progress (21.1-21.2 complete)
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
- **21.3 Identity provider seam**: OIDC device-code login populating
  subjects and group membership, with the static token map kept as the
  offline path; `converge login` gains a browser flow
- **21.4 Adversarial**: an expired token is refused everywhere, a
  revoked one immediately, a scoped one cannot exceed its scope, and no
  path reintroduces a long-lived unscoped credential

## Exit Criteria

- every token has an expiry and a revocation path that does not involve
  SQL
- a token can be narrower than its subject
- an organisation can bring its own accounts without Convergence storing
  a password
- doc 14 §4's "deferred" identity note is replaced by what is built

## Next Task

Open batch card 21.3 (identity provider seam).
