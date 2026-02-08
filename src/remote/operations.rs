//! Repo/gate/bundle/release/promotion administrative operations.

use super::*;

impl RemoteClient {
    pub fn create_repo(&self, repo_id: &str) -> Result<Repo> {
        let resp = self
            .client
            .post(self.url("/repos"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&CreateRepoRequest {
                id: repo_id.to_string(),
            })
            .send()
            .context("create repo request")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote endpoint not found (is converge-server running?)");
        }

        let resp = self.ensure_ok(resp, "create repo")?;
        let repo: Repo = resp.json().context("parse create repo response")?;
        Ok(repo)
    }

    pub fn list_publications(&self) -> Result<Vec<Publication>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/publications", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list publications")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let pubs: Vec<Publication> = self
            .ensure_ok(resp, "list publications")?
            .json()
            .context("parse publications")?;
        Ok(pubs)
    }

    pub fn get_gate_graph(&self) -> Result<GateGraph> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/gate-graph", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get gate graph")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let graph: GateGraph = self
            .ensure_ok(resp, "get gate graph")?
            .json()
            .context("parse gate graph")?;
        Ok(graph)
    }

    pub fn put_gate_graph(&self, graph: &GateGraph) -> Result<GateGraph> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .put(self.url(&format!("/repos/{}/gate-graph", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(graph)
            .send()
            .context("put gate graph")?;

        if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            let v: GateGraphValidationError =
                resp.json().context("parse gate graph validation error")?;
            if v.issues.is_empty() {
                anyhow::bail!(v.error);
            }

            let mut lines: Vec<String> = Vec::new();
            lines.push(v.error);
            for i in v.issues.iter().take(8) {
                let mut bits = Vec::new();
                bits.push(i.code.clone());
                if let Some(g) = &i.gate {
                    bits.push(format!("gate={}", g));
                }
                if let Some(u) = &i.upstream {
                    bits.push(format!("upstream={}", u));
                }
                lines.push(format!("- {}: {}", bits.join(" "), i.message));
            }
            if v.issues.len() > 8 {
                lines.push(format!("... and {} more", v.issues.len() - 8));
            }
            anyhow::bail!(lines.join("\n"));
        }
        let graph: GateGraph = self
            .ensure_ok(resp, "put gate graph")?
            .json()
            .context("parse gate graph")?;
        Ok(graph)
    }

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
