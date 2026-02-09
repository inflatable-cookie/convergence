use super::*;

impl RemoteClient {
    pub fn list_pins(&self) -> Result<Pins> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/pins", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list pins")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let pins: Pins = self
            .ensure_ok(resp, "list pins")?
            .json()
            .context("parse pins")?;
        Ok(pins)
    }

    pub fn pin_bundle(&self, bundle_id: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/bundles/{}/pin", repo, bundle_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("pin bundle")?;

        let _ = self.ensure_ok(resp, "pin bundle")?;
        Ok(())
    }

    pub fn unpin_bundle(&self, bundle_id: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/bundles/{}/unpin", repo, bundle_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("unpin bundle")?;

        let _ = self.ensure_ok(resp, "unpin bundle")?;
        Ok(())
    }
}
