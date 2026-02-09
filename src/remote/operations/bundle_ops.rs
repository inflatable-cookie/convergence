use super::*;

impl RemoteClient {
    pub fn create_bundle(
        &self,
        scope: &str,
        gate: &str,
        publications: &[String],
    ) -> Result<Bundle> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/bundles", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "scope": scope,
                "gate": gate,
                "input_publications": publications
            }))
            .send()
            .context("create bundle request")?;
        let resp = self.ensure_ok(resp, "create bundle")?;
        let bundle: Bundle = resp.json().context("parse bundle")?;
        Ok(bundle)
    }

    pub fn list_bundles(&self) -> Result<Vec<Bundle>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/bundles", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list bundles")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let bundles: Vec<Bundle> = self
            .ensure_ok(resp, "list bundles")?
            .json()
            .context("parse bundles")?;
        Ok(bundles)
    }

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

    pub fn get_bundle(&self, bundle_id: &str) -> Result<Bundle> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/bundles/{}", repo, bundle_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get bundle")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("bundle not found");
        }

        let bundle: Bundle = self
            .ensure_ok(resp, "get bundle")?
            .json()
            .context("parse bundle")?;
        Ok(bundle)
    }

    pub fn approve_bundle(&self, bundle_id: &str) -> Result<Bundle> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/bundles/{}/approve", repo, bundle_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("approve request")?;

        let resp = self.ensure_ok(resp, "approve")?;

        let bundle: Bundle = resp.json().context("parse approved bundle")?;
        Ok(bundle)
    }
}
