//! Identity: tokens, keys, probes, provider exchange.

use anyhow::{Context, Result};

use super::Probe;

use super::RemoteClient;

impl RemoteClient {
    /// Register a public key for the calling subject (batch 19.1).
    pub fn register_key(
        &self,
        repo_id: &str,
        public_key: &str,
        label: &str,
    ) -> Result<crate::model::PublicKeyRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/keys")))
                .bearer_auth(&self.token)
                .json(&crate::model::RegisterKeyRequest {
                    public_key: public_key.into(),
                    label: label.into(),
                })
                .send()
                .context("register key")?,
        )?;
        response.json().context("parse key record")
    }

    /// One round trip that answers "is the server there, does my token
    /// work, and do our clocks agree" (g02.022 batch 22.1).
    ///
    /// Deliberately not three calls: a diagnostic that reports
    /// reachability, then authentication, then skew, from three separate
    /// requests can describe a state that never existed at one moment.
    pub fn probe(&self, repo_id: &str) -> Probe {
        // An authenticated route, so the same response answers both
        // "reachable" and "does this credential work". `lanes` needs
        // only `read`, which is the narrowest thing any member holds.
        let sent_at = time::OffsetDateTime::now_utc();
        let response = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/lanes")))
            .bearer_auth(&self.token)
            .send();
        let round_trip: time::Duration = time::OffsetDateTime::now_utc() - sent_at;
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                return Probe {
                    reachable: false,
                    detail: format!("{err}"),
                    ..Probe::default()
                };
            }
        };
        // The `Date` header is the server's own clock, which is the only
        // clock worth comparing against: batch 21.3's identity exchange
        // refuses a token 60 seconds out, and blames the token.
        let skew_seconds = response
            .headers()
            .get(reqwest::header::DATE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc2822)
                    .ok()
            })
            .map(|server_now: time::OffsetDateTime| {
                // Charge the round trip to the server's favour: half of
                // it elapsed before the header was written, so a slow
                // link should not read as a wrong clock.
                let local_now = sent_at + round_trip / 2i32;
                (server_now - local_now).whole_seconds()
            });
        let status = response.status();
        Probe {
            reachable: true,
            authenticated: status != reqwest::StatusCode::UNAUTHORIZED,
            authorized: status.is_success(),
            status: Some(status.as_u16()),
            skew_seconds,
            detail: response.text().unwrap_or_default(),
        }
    }

    pub fn list_keys(&self, repo_id: &str) -> Result<Vec<crate::model::PublicKeyRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/keys")))
                .bearer_auth(&self.token)
                .send()
                .context("list keys")?,
        )?;
        response.json().context("parse keys")
    }

    /// What this server accepts for sign-in (batch 21.3). No token
    /// needed: a client has to ask this before it has one.
    pub fn auth_config(base_url: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/auth/config", base_url.trim_end_matches('/'));
        let response = reqwest::blocking::get(&url).context("read auth config")?;
        response.json().context("parse auth config")
    }

    /// Trade a provider-issued identity token for a Convergence one.
    pub fn exchange_identity(base_url: &str, id_token: &str) -> Result<crate::model::TokenIssued> {
        let url = format!("{}/api/auth/exchange", base_url.trim_end_matches('/'));
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .json(&crate::model::ExchangeIdentityRequest {
                id_token: id_token.into(),
            })
            .send()
            .context("exchange identity token")?;
        Self::check(response)?.json().context("parse issued token")
    }

    /// Issue a token for the calling subject, narrower than they are.
    pub fn issue_token(
        &self,
        repo_id: &str,
        label: &str,
        capabilities: &[String],
        expires_in_days: Option<u32>,
    ) -> Result<crate::model::TokenIssued> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/tokens")))
                .bearer_auth(&self.token)
                .json(&crate::model::IssueTokenRequest {
                    label: label.into(),
                    capabilities: capabilities.to_vec(),
                    expires_in_days,
                })
                .send()
                .context("issue token")?,
        )?;
        response.json().context("parse issued token")
    }

    pub fn list_tokens(&self, repo_id: &str) -> Result<Vec<crate::model::TokenRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/tokens")))
                .bearer_auth(&self.token)
                .send()
                .context("list tokens")?,
        )?;
        response.json().context("parse tokens")
    }

    pub fn revoke_token(
        &self,
        repo_id: &str,
        token_id: &str,
        reason: &str,
    ) -> Result<crate::model::TokenRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/tokens/{token_id}/revoke")))
                .bearer_auth(&self.token)
                .json(&crate::model::RevokeTokenRequest {
                    reason: reason.into(),
                })
                .send()
                .context("revoke token")?,
        )?;
        response.json().context("parse token")
    }
}
