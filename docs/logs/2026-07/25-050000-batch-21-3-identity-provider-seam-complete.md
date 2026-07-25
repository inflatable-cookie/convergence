# Batch 21.3 — Identity Provider Seam Complete

Date: 2026-07-25
Roadmap: `g02.021`
Card: `080-identity-provider-seam`

## What Shipped

An organisation can bring its own accounts. With an issuer configured,
`GET /api/auth/config` advertises it, `POST /api/auth/exchange` verifies
an RS256 identity token against the issuer's JWKS — signature, issuer,
audience, expiry — and mints a Convergence token with a real lifetime
from 21.1. `converge login --oidc` runs the device-code flow. Static
`--token` pairs stay as the offline path.

Convergence is not an OIDC client. It verifies one issuer's assertions
and stops there: the browser dance belongs where a browser already is,
and a server owning refresh cycles and provider quirks would be a second
identity system rather than a seam.

Signing in grants nothing. A first login provisions a subject with **no
grants**, and the test proves it by asserting the exchanged token is
refused *403* rather than 401 — authentication succeeded, authorization
declined, which is the right order of failure.

Every refusal names which check failed. Expired, wrong issuer, and wrong
audience are three different messages, because "invalid token" is the
least useful thing to tell someone at a login prompt.

## Three Defects The Test Found

Writing the test against a fake issuer — one that mints its own RS256
tokens and serves its own JWKS — found three failures that a manual test
against a live provider would have hidden or misattributed:

1. **No crypto provider.** `jsonwebtoken` 11 selects none by default and
   panics on the first verification. The build was green; every exchange
   would have died at runtime. Pinned to `rust_crypto` (pure Rust, no C
   toolchain), with the reason recorded in `Cargo.toml` so nobody
   "cleans up" the feature later.
2. **Blocking fetch on the async worker.** The JWKS refresh is blocking,
   and running it in the handler built a runtime inside a runtime. The
   connection aborted mid-response, so a client would have seen a
   transport error rather than a refusal. It now runs on a blocking
   thread.
3. **Inherited clock skew.** `jsonwebtoken` allows 60 seconds of leeway
   by default, so a token 60 seconds past expiry still verified. The
   leeway is now stated and deliberate: issuer and server clocks drift,
   and a token living a minute past its expiry costs less than refusing
   valid logins on a badly-synced host.

## Docs

Doc 14 §4's identity deferral is gone, replaced by §4b describing what
21.1–21.3 actually built. The deferred-work table now names the real
remaining gap — mapping IdP groups to capabilities — instead of the
token lifecycle that now exists.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets --all-features -D warnings`: clean
- `cargo nextest run --workspace`: 240 passed, 4 skipped

## Next Task

Batch card 21.4 (adversarial), closing `g02.021`.
