use super::*;

impl RemoteClient {
    pub fn list_releases(&self) -> Result<Vec<Release>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/releases", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list releases")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let releases: Vec<Release> = self
            .ensure_ok(resp, "list releases")?
            .json()
            .context("parse releases")?;
        Ok(releases)
    }

    pub fn get_release(&self, channel: &str) -> Result<Release> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/releases/{}", repo, channel)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get release")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("release not found");
        }

        let r: Release = self
            .ensure_ok(resp, "get release")?
            .json()
            .context("parse release")?;
        Ok(r)
    }

    pub fn create_release(
        &self,
        channel: &str,
        bundle_id: &str,
        notes: Option<String>,
    ) -> Result<Release> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/releases", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "channel": channel,
                "bundle_id": bundle_id,
                "notes": notes,
            }))
            .send()
            .context("create release")?;

        let resp = self.ensure_ok(resp, "create release")?;
        let r: Release = resp.json().context("parse release")?;
        Ok(r)
    }
}
