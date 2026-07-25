# 080 Identity Provider Seam

Status: complete
Updated: 2026-07-25
Roadmap: `g02.021`

## Objective

Let an organisation bring its own accounts: exchange an OIDC identity
token for a Convergence token, so people log in with the identity they
already have.

## Scope of the actual problem

Subjects arrive one of two ways today: a `--token subject=value` startup
flag, or an admin running `member add`. Both mean Convergence is the
account system, which is exactly what an organisation with SSO does not
want and cannot audit.

The design question is where the boundary sits. Embedding a full OIDC
client in the server would make Convergence responsible for browser
flows, refresh cycles, and provider quirks. The narrower seam is the
valuable one: the server *verifies an assertion from a configured
issuer* and exchanges it for a Convergence token. The interactive dance
belongs to the client, where a browser already is.

One thing this deliberately does **not** do is grant anything. SSO
establishes who you are; it does not say what you may do. A first login
provisions a subject with no capabilities, and an admin grants — which
is the correct posture and worth stating, because the opposite
(everyone in the directory becomes a member) is a common and expensive
default.

## In Scope

- server config for a trusted issuer: issuer URL, audience, and the
  claim to read as the subject
- `POST /api/auth/exchange`: verify an RS256 identity token against the
  issuer's JWKS, check issuer, audience and expiry, provision the
  subject, and mint a Convergence token
- `GET /api/auth/config` so a client can discover the issuer
- `converge login --oidc` running the device-code flow and exchanging
  the result
- static `--token` pairs kept as the offline path

## Out Of Scope

- mapping IdP groups to capabilities: identity is not authorization, and
  the mapping deserves its own decision rather than a default
- refresh tokens: the Convergence token has its own lifetime from 21.1,
  which is the thing that matters here
- provider-specific behaviour beyond standard discovery

## Acceptance Criteria

- a valid token from the configured issuer yields a Convergence token
  and a provisioned subject with no grants; a token from another issuer,
  for another audience, expired, or wrongly signed is refused with a
  distinct reason; a server without OIDC configured says so; all suites
  green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- the seam is built as designed: `GET /api/auth/config` advertises the
  issuer, `POST /api/auth/exchange` verifies an RS256 assertion against
  the issuer's JWKS and mints a Convergence token with a real expiry,
  and `converge login --oidc` runs the device-code flow. Convergence is
  not an OIDC client; it verifies one issuer's assertions
- **signing in grants nothing.** The test asserts the exchanged token
  authenticates and is then refused *403*, not 401 — the right order of
  failure, and the proof that identity did not become authorization
- every refusal names which check failed. "Invalid token" is the least
  useful thing to tell someone at a login prompt, so expired, wrong
  issuer, and wrong audience are three different messages
- writing the test against a fake issuer found **three real defects**
  that a manual test against a live provider would have hidden:
  - `jsonwebtoken` 11 selects **no crypto provider by default** and
    panics on the first verification. Every exchange would have died at
    runtime while the build stayed green. Now pinned to `rust_crypto`,
    with the reason in `Cargo.toml`
  - the JWKS fetch is blocking, and running it on the async worker built
    a runtime inside a runtime — the connection aborted mid-response, so
    the client saw a transport error rather than a refusal. It now runs
    on a blocking thread
  - `jsonwebtoken` allows 60 seconds of clock skew by default, so a
    token 60 seconds past expiry still verified. The leeway is now
    stated and deliberate: issuer and server clocks drift, and a token
    living a minute long costs less than refusing valid logins
- 240 tests green

## Next Task

Batch card 21.4 (adversarial).
