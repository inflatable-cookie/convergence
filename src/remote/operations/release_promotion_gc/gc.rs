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
}
