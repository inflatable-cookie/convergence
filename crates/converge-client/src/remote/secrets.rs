//! Encrypted secrets.

use anyhow::{Context, Result};

use super::RemoteClient;

impl RemoteClient {
    /// Store ciphertext (batch 19.2). `expected_version` is the version
    /// being replaced; 0 creates.
    pub fn set_secret(
        &self,
        repo_id: &str,
        name: &str,
        ciphertext: &str,
        recipients: &[String],
        expected_version: u64,
    ) -> Result<crate::model::SecretSummary> {
        self.write_secret(
            repo_id,
            name,
            ciphertext,
            recipients,
            expected_version,
            true,
        )
    }

    /// As `set_secret`, declaring whether the *value* changed so an
    /// audit can tell a rotation from a re-share (batch 20.3).
    pub fn write_secret(
        &self,
        repo_id: &str,
        name: &str,
        ciphertext: &str,
        recipients: &[String],
        expected_version: u64,
        value_changed: bool,
    ) -> Result<crate::model::SecretSummary> {
        let response = Self::check(
            self.http
                .put(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
                .bearer_auth(&self.token)
                .json(&crate::model::SetSecretRequest {
                    ciphertext: ciphertext.into(),
                    recipients: recipients.to_vec(),
                    expected_version,
                    value_changed,
                })
                .send()
                .context("set secret")?,
        )?;
        response.json().context("parse secret summary")
    }

    pub fn get_secret(&self, repo_id: &str, name: &str) -> Result<crate::model::SecretRecord> {
        self.get_secret_owned(repo_id, name, None)
    }

    /// `owner` disambiguates when two people hold the same name
    /// (batch 20.1).
    pub fn get_secret_owned(
        &self,
        repo_id: &str,
        name: &str,
        owner: Option<&str>,
    ) -> Result<crate::model::SecretRecord> {
        let mut request = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
            .bearer_auth(&self.token);
        if let Some(owner) = owner {
            request = request.query(&[("owner", owner)]);
        }
        let response = Self::check(request.send().context("get secret")?)?;
        response.json().context("parse secret")
    }

    pub fn list_secrets(&self, repo_id: &str) -> Result<Vec<crate::model::SecretSummary>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/secrets")))
                .bearer_auth(&self.token)
                .send()
                .context("list secrets")?,
        )?;
        response.json().context("parse secrets")
    }

    pub fn delete_secret(&self, repo_id: &str, name: &str) -> Result<()> {
        Self::check(
            self.http
                .delete(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
                .bearer_auth(&self.token)
                .send()
                .context("delete secret")?,
        )?;
        Ok(())
    }
}
