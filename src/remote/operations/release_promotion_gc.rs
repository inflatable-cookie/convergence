use super::*;

impl RemoteClient {
    pub fn gc_repo(
        &self,
        dry_run: bool,
        prune_metadata: bool,
        prune_releases_keep_last: Option<usize>,
    ) -> Result<serde_json::Value> {
        let repo = &self.remote.repo_id;

        let mut url = self.url(&format!(
            "/repos/{}/gc?dry_run={}&prune_metadata={}",
            repo, dry_run, prune_metadata
        ));
        if let Some(n) = prune_releases_keep_last {
            url.push_str(&format!("&prune_releases_keep_last={}", n));
        }

        let resp = self
            .client
            .post(url)
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("gc repo")?;

        let v: serde_json::Value = self
            .ensure_ok(resp, "gc repo")?
            .json()
            .context("parse gc response")?;
        Ok(v)
    }

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

    pub fn promote_bundle(&self, bundle_id: &str, to_gate: &str) -> Result<Promotion> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/promotions", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "bundle_id": bundle_id,
                "to_gate": to_gate
            }))
            .send()
            .context("promote request")?;
        let resp = self.ensure_ok(resp, "promote")?;
        let promotion: Promotion = resp.json().context("parse promotion")?;
        Ok(promotion)
    }

    pub fn promotion_state(&self, scope: &str) -> Result<HashMap<String, String>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/promotion-state?scope={}", repo, scope)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("promotion state request")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let resp = self.ensure_ok(resp, "promotion state")?;

        let state: HashMap<String, String> = resp.json().context("parse promotion state")?;
        Ok(state)
    }
}
