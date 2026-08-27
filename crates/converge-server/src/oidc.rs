//! Identity provider seam (g02.021 batch 21.3, doc 14 §4).
//!
//! Convergence verifies an assertion from one configured issuer and
//! exchanges it for a Convergence token. It is not an OIDC client: the
//! browser dance belongs where a browser already is, and a server that
//! owned refresh cycles and provider quirks would be a second identity
//! system rather than a seam.
//!
//! Verification establishes *who*, never *what they may do*. A first
//! login provisions a subject with no grants, because "everyone in the
//! directory is a member" is a default nobody can afford.

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// A trusted issuer, as an operator configures it.
#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer: String,
    pub audience: String,
    /// Claim read as the Convergence subject. `preferred_username` and
    /// `email` are the usual choices; `sub` is stable but unreadable.
    pub subject_claim: String,
}

/// Claims Convergence cares about. Anything else the provider sends is
/// ignored rather than trusted.
#[derive(Debug, Deserialize)]
struct Claims {
    iss: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    preferred_username: String,
    #[serde(default)]
    name: String,
}

impl Claims {
    fn subject_for(&self, claim: &str) -> Option<&str> {
        let value = match claim {
            "sub" => &self.sub,
            "email" => &self.email,
            "preferred_username" => &self.preferred_username,
            "name" => &self.name,
            _ => return None,
        };
        (!value.is_empty()).then_some(value.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    #[serde(default)]
    n: String,
    #[serde(default)]
    e: String,
}

/// Verifier for one issuer, caching its signing keys.
pub struct OidcVerifier {
    config: OidcConfig,
    keys: Mutex<Vec<Jwk>>,
}

impl std::fmt::Debug for OidcVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcVerifier")
            .field("issuer", &self.config.issuer)
            .field("audience", &self.config.audience)
            .field("subject_claim", &self.config.subject_claim)
            .field(
                "cached_key_count",
                &self.keys.lock().map(|keys| keys.len()).unwrap_or(0),
            )
            .finish_non_exhaustive()
    }
}

impl OidcVerifier {
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            keys: Mutex::new(Vec::new()),
        }
    }

    pub fn issuer(&self) -> &str {
        &self.config.issuer
    }

    pub fn audience(&self) -> &str {
        &self.config.audience
    }

    /// Verify an identity token and return the subject it names.
    ///
    /// Every check is explicit: signature against the issuer's published
    /// key, issuer match, audience match, and expiry. A token that fails
    /// any of them is refused with which one, because "invalid token" is
    /// the least useful thing to tell someone at a login prompt.
    pub fn subject_from(&self, id_token: &str) -> Result<String> {
        let header = jsonwebtoken::decode_header(id_token).context("parse token header")?;
        let key = self
            .signing_key(header.kid.as_deref())
            .context("no signing key from the issuer matched this token")?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[self.config.issuer.as_str()]);
        validation.set_audience(&[self.config.audience.as_str()]);
        // Expiry is validated by default; state it so the absence of a
        // line disabling it is visibly deliberate.
        validation.validate_exp = true;
        // So is a 60-second leeway, which is a decision rather than a
        // default worth inheriting silently: clocks between an issuer
        // and this server do drift, and the cost of a token living a
        // minute past its stated expiry is smaller than the cost of
        // refusing valid logins on a badly-synced host.
        validation.leeway = 60;

        let decoded =
            jsonwebtoken::decode::<Claims>(id_token, &key, &validation).map_err(|err| match err
                .kind()
            {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    anyhow!("identity token has expired; sign in again")
                }
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => anyhow!(
                    "identity token is from another issuer; this server trusts {}",
                    self.config.issuer
                ),
                jsonwebtoken::errors::ErrorKind::InvalidAudience => anyhow!(
                    "identity token is for another audience; this server expects {}",
                    self.config.audience
                ),
                _ => anyhow!("identity token failed verification: {err}"),
            })?;

        if decoded.claims.iss != self.config.issuer {
            anyhow::bail!("identity token issuer does not match");
        }
        let subject = decoded
            .claims
            .subject_for(&self.config.subject_claim)
            .ok_or_else(|| {
                anyhow!(
                    "identity token carries no `{}` claim to use as a subject",
                    self.config.subject_claim
                )
            })?;
        Ok(subject.to_string())
    }

    fn signing_key(&self, kid: Option<&str>) -> Result<jsonwebtoken::DecodingKey> {
        if let Some(key) = self.lookup(kid) {
            return Ok(key);
        }
        // Miss: the provider may have rotated. Refresh once, then fail.
        self.refresh_keys()?;
        self.lookup(kid)
            .ok_or_else(|| anyhow!("issuer published no key with id {kid:?}"))
    }

    fn lookup(&self, kid: Option<&str>) -> Option<jsonwebtoken::DecodingKey> {
        let keys = self.keys.lock().expect("jwks lock");
        let jwk = keys
            .iter()
            .find(|k| match (kid, k.kid.as_deref()) {
                (Some(wanted), Some(have)) => wanted == have,
                // A provider publishing one unlabelled key is common
                // enough to accept rather than reject on principle.
                (_, None) => true,
                (None, Some(_)) => keys.len() == 1,
            })?
            .clone();
        jsonwebtoken::DecodingKey::from_rsa_components(&jwk.n, &jwk.e).ok()
    }

    fn refresh_keys(&self) -> Result<()> {
        let url = format!(
            "{}/.well-known/jwks.json",
            self.config.issuer.trim_end_matches('/')
        );
        let response = reqwest::blocking::get(&url)
            .with_context(|| format!("fetch signing keys from {url}"))?;
        if !response.status().is_success() {
            anyhow::bail!("issuer returned {} for its signing keys", response.status());
        }
        let jwks: Jwks = response.json().context("parse issuer signing keys")?;
        *self.keys.lock().expect("jwks lock") = jwks.keys;
        Ok(())
    }
}
