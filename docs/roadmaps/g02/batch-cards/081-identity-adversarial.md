# 081 Identity Adversarial

Status: complete
Updated: 2026-07-25
Roadmap: `g02.021`

## Objective

Try to get in with a credential that should not work, and try to keep
access after it has been taken away.

## Scope of the actual problem

21.1–21.3 built three mechanisms that all say "no" for different
reasons: expiry, revocation, and scope. Each was tested where it was
built. What has *not* been tested is whether they hold on every path.

The specific worry is coverage rather than logic. Authentication runs in
one place, but the paths that reach it do not: the secret endpoints, the
token endpoints, the object and event routes, and the identity exchange
each arrive with their own assumptions. A check that lives in a shared
extractor is only as good as the routes that go through it, and a route
that authenticates by hand — or one whose handler runs before the check
— is how expired credentials survive.

The second worry is the ordering the whole design rests on: 21.3 asserts
that authentication succeeding and authorization declining is a *403*.
If any path returns 401 where it means 403, an operator debugging a
failed CI job will go looking for a broken token instead of a missing
grant. The reverse is worse: a 403 where the credential itself is
invalid reads as "you exist but may not", which is a lie.

Third: revocation must be immediate, not eventual. Any cache of
token→subject — including one added for performance later — turns
revocation into a suggestion.

## In Scope

- an expired token refused on **every** authenticated route, not just
  the ones 21.1 tested
- a revoked token refused on the next request, with no grace
- a scoped token refused for capabilities outside its scope on every
  route that consumes one, and accepted for everything inside it
- a scoped token cannot mint a wider one, directly or by chaining
  through a second issue
- the exchanged-token path inherits all of the above: OIDC is not a way
  around expiry, revocation, or scope
- refusals distinguish 401 from 403 consistently across routes
- a token for one repo cannot act in another; a site-admin grant is not
  reachable by a repo grant

## Out Of Scope

- rate limiting and brute-force defence: a real concern, a different
  mechanism, and one that belongs with deployment rather than identity
- provider-side compromise: if the issuer signs a lie, Convergence
  believes it, and that is the stated trust boundary

## Acceptance Criteria

- an adversarial suite covering the above, every case failing closed;
  any gap found is fixed rather than documented; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

The coverage worry was correct, and the table found what a hand-picked
list would not have.

- **scope was a property of the route, not the token.** Batch 21.2 put
  the scope check inside `authorize_repo` and left the twenty handlers
  that called `authorize` directly untouched. `POST members` was one of
  them, so a **read-scoped token could add a member with any
  capabilities — including granting itself admin**. Every handler now
  goes through one entry point, `authorize_scoped`, which checks scope
  then grant; `authorize` is no longer called from any handler. The two
  remaining call sites are that entry point and `issue_token`, which
  does its own explicit double check
- `site_admin` had the same gap: creating repos is the widest operation
  there is, and it consulted the subject without consulting the token.
  It now checks scope first
- **authentication ran after body parsing.** Axum's `Json` extractor
  runs before the handler, so an anonymous caller reached the request
  parser on every route, could read the schema out of a 422, and could
  push a body up to the 64 MiB limit through it before anyone asked who
  they were. A layer now authenticates before routing. Handlers still
  authenticate — they need the subject, and a check that exists only in
  a layer is one route registration away from being skipped — so what
  the layer adds is ordering, not the check itself
- the suite is table-driven over **all 41 authenticated routes**, with a
  control test asserting the same routes accept a live admin token. Both
  halves are needed: without the control, a route that failed for its
  own reasons would look like a route that was checking
- revocation is immediate: the test revokes between two requests on the
  same connection and asserts the second is refused, which is what would
  catch a token cache added later for performance
- 401 and 403 stay distinct — unknown token versus grantless subject —
  and the scope refusal names the scope rather than blaming the grant
- a repo admin reaches neither another repo nor the site-admin
  operations, so nothing here made a repo grant a route to `POST /repos`

## Next Task

Close `g02.021`; open roadmap `g02.023` with batch card 23.1.
