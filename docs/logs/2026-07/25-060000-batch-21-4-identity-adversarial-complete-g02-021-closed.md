# Batch 21.4 — Identity Adversarial Complete, g02.021 Closed

Date: 2026-07-25
Roadmap: `g02.021`
Card: `081-identity-adversarial`

## What The Suite Does

Table-driven over **all 41 authenticated routes**, rather than a
hand-picked few. The failure worth catching here is a route nobody
remembered to check, and a list of memorable routes catches none of
those.

Two halves, both needed. The first drives every route with an expired
token and a revoked one and demands 401 with the reason named. The
second drives the same routes with a live admin token and demands *not*
401 — without it, a route that failed for its own reasons would look
like a route that was checking.

## Two Real Findings

**Scope was a property of the route, not the token.** Batch 21.2 put the
scope check inside `authorize_repo` and left the twenty handlers that
called `authorize` directly untouched. `POST /api/repos/:repo/members`
was one of them: a **read-scoped token could add a member with any
capabilities, including granting itself admin**. The scope mechanism
looked correct everywhere it was tested and was absent everywhere it was
not.

Fixed by removing the choice. Every handler now goes through one entry
point, `authorize_scoped`, which checks scope and then the grant.
`authorize` is called from exactly two places: that entry point, and
`issue_token`, which does its own explicit double check. `site_admin`
had the same gap — creating repos is the widest operation there is and
it consulted the subject without consulting the token — and now checks
scope first.

**Authentication ran after body parsing.** Axum runs the `Json`
extractor before the handler, and authentication lived in the handler.
So an anonymous caller reached the request parser on every route, could
read the schema out of a 422, and could push a body up to the 64 MiB
limit through it before anyone asked who they were. A layer now
authenticates before routing. Handlers still authenticate: they need the
subject, and a check that exists only in a layer is one route
registration away from being skipped. What the layer adds is ordering.

## Also Asserted

- revocation takes effect on the very next request, which is the test
  that would catch a token cache added later for performance
- 401 and 403 stay distinct: unknown token versus grantless subject.
  Conflating them sends whoever is debugging a failed job looking for a
  broken token instead of a missing grant
- a scope refusal names the scope rather than blaming the grant
- `secret list` still works on a read-scoped token: scope is precise
  rather than blunt, and that is the case that would regress if someone
  "tightened" it
- a repo admin reaches neither another repo nor `POST /api/repos`

## g02.021 Closed

All four exit criteria met. Doc 14 §4's identity deferral is gone,
replaced by §4b — what is built, plus where the checks live and why the
single-entry-point rule is load-bearing.

The pattern across this roadmap is worth naming: the two batches that
*tested* rather than built found the most. 21.3 found three defects that
would have made every exchange fail at runtime, and 21.4 found an
escalation that 21.2 had shipped.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets --all-features -D warnings`: clean
- `cargo nextest run --workspace`: 247 passed, 4 skipped

## Next Task

Open roadmap `g02.023` (TUI completion) with batch card 23.1: reality
check and simplification sweep.
