use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};

use crate::model::{ObjectId, RemoteConfig, SnapRecord};
use crate::store::LocalStore;

mod http_client;
use self::http_client::with_retries;

mod types;
pub use self::types::*;
mod fetch;
use self::fetch::*;

pub struct RemoteClient {
    remote: RemoteConfig,
    token: String,
    client: reqwest::blocking::Client,
}

impl RemoteClient {
    pub fn new(remote: RemoteConfig, token: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("converge")
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            remote,
            token,
            client,
        })
    }

    pub fn remote(&self) -> &RemoteConfig {
        &self.remote
    }

    pub fn whoami(&self) -> Result<WhoAmI> {
        let resp = self
            .client
            .get(self.url("/whoami"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("whoami")?;
        let w: WhoAmI = self
            .ensure_ok(resp, "whoami")?
            .json()
            .context("parse whoami")?;
        Ok(w)
    }

    pub fn bootstrap_first_admin(
        &self,
        handle: &str,
        display_name: Option<String>,
    ) -> Result<BootstrapResponse> {
        let resp = self
            .client
            .post(self.url("/bootstrap"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "handle": handle,
                "display_name": display_name,
            }))
            .send()
            .context("bootstrap")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "bootstrap endpoint not enabled (start converge-server with --bootstrap-token and an empty data dir)"
            );
        }
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("unauthorized (bootstrap token invalid)");
        }
        if resp.status() == reqwest::StatusCode::CONFLICT {
            let v: serde_json::Value = resp.json().context("parse bootstrap error")?;
            let msg = v
                .get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("already bootstrapped");
            anyhow::bail!(msg.to_string());
        }

        let out: BootstrapResponse = resp
            .error_for_status()
            .with_context(|| "bootstrap status".to_string())?
            .json()
            .context("parse bootstrap response")?;
        Ok(out)
    }

    pub fn get_repo(&self, repo_id: &str) -> Result<Repo> {
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}", repo_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get repo")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote repo not found");
        }

        let repo: Repo = self
            .ensure_ok(resp, "get repo")?
            .json()
            .context("parse repo")?;
        Ok(repo)
    }

    pub fn list_users(&self) -> Result<Vec<RemoteUser>> {
        let resp = self
            .client
            .get(self.url("/users"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list users")?;
        let out: Vec<RemoteUser> = self
            .ensure_ok(resp, "list users")?
            .json()
            .context("parse users")?;
        Ok(out)
    }

    pub fn create_user(
        &self,
        handle: &str,
        display_name: Option<String>,
        admin: bool,
    ) -> Result<RemoteUser> {
        let resp = self
            .client
            .post(self.url("/users"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "handle": handle,
                "display_name": display_name,
                "admin": admin
            }))
            .send()
            .context("create user")?;
        let out: RemoteUser = self
            .ensure_ok(resp, "create user")?
            .json()
            .context("parse create user")?;
        Ok(out)
    }

    pub fn create_token_for_user(
        &self,
        user_id: &str,
        label: Option<String>,
    ) -> Result<CreateTokenResponse> {
        let resp = self
            .client
            .post(self.url(&format!("/users/{}/tokens", user_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"label": label}))
            .send()
            .context("create token for user")?;
        let out: CreateTokenResponse = self
            .ensure_ok(resp, "create token for user")?
            .json()
            .context("parse create token for user")?;
        Ok(out)
    }

    pub fn list_tokens(&self) -> Result<Vec<TokenView>> {
        let resp = self
            .client
            .get(self.url("/tokens"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list tokens")?;
        let out: Vec<TokenView> = self
            .ensure_ok(resp, "list tokens")?
            .json()
            .context("parse tokens")?;
        Ok(out)
    }

    pub fn create_token(&self, label: Option<String>) -> Result<CreateTokenResponse> {
        let resp = self
            .client
            .post(self.url("/tokens"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"label": label}))
            .send()
            .context("create token")?;
        let out: CreateTokenResponse = self
            .ensure_ok(resp, "create token")?
            .json()
            .context("parse create token")?;
        Ok(out)
    }

    pub fn revoke_token(&self, token_id: &str) -> Result<()> {
        let resp = self
            .client
            .post(self.url(&format!("/tokens/{}/revoke", token_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("revoke token")?;
        let _ = self.ensure_ok(resp, "revoke token")?;
        Ok(())
    }

    pub fn list_repo_members(&self) -> Result<RepoMembers> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/members", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list repo members")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote repo not found");
        }

        let out: RepoMembers = self
            .ensure_ok(resp, "list repo members")?
            .json()
            .context("parse repo members")?;
        Ok(out)
    }

    pub fn add_repo_member(&self, handle: &str, role: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/members", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"handle": handle, "role": role}))
            .send()
            .context("add repo member")?;

        let _ = self.ensure_ok(resp, "add repo member")?;
        Ok(())
    }

    pub fn remove_repo_member(&self, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .delete(self.url(&format!("/repos/{}/members/{}", repo, handle)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("remove repo member")?;
        let _ = self.ensure_ok(resp, "remove repo member")?;
        Ok(())
    }

    pub fn list_lane_members(&self, lane_id: &str) -> Result<LaneMembers> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes/{}/members", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list lane members")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote lane not found");
        }

        let out: LaneMembers = self
            .ensure_ok(resp, "list lane members")?
            .json()
            .context("parse lane members")?;
        Ok(out)
    }

    pub fn add_lane_member(&self, lane_id: &str, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/lanes/{}/members", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"handle": handle}))
            .send()
            .context("add lane member")?;

        let _ = self.ensure_ok(resp, "add lane member")?;
        Ok(())
    }

    pub fn remove_lane_member(&self, lane_id: &str, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .delete(self.url(&format!(
                "/repos/{}/lanes/{}/members/{}",
                repo, lane_id, handle
            )))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("remove lane member")?;
        let _ = self.ensure_ok(resp, "remove lane member")?;
        Ok(())
    }

    pub fn list_lanes(&self) -> Result<Vec<Lane>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list lanes")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let lanes: Vec<Lane> = self
            .ensure_ok(resp, "list lanes")?
            .json()
            .context("parse lanes")?;
        Ok(lanes)
    }

    pub fn update_lane_head_me(
        &self,
        lane_id: &str,
        snap_id: &str,
        client_id: Option<String>,
    ) -> Result<LaneHead> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/lanes/{}/heads/me", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&UpdateLaneHeadRequest {
                snap_id: snap_id.to_string(),
                client_id,
            })
            .send()
            .context("update lane head")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote lane not found (check `converge lanes` or /repos/:repo/lanes)");
        }

        let head: LaneHead = self
            .ensure_ok(resp, "update lane head")?
            .json()
            .context("parse lane head")?;
        Ok(head)
    }

    pub fn get_lane_head(&self, lane_id: &str, user: &str) -> Result<LaneHead> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes/{}/heads/{}", repo, lane_id, user)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get lane head")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("lane head not found");
        }

        let head: LaneHead = self
            .ensure_ok(resp, "get lane head")?
            .json()
            .context("parse lane head")?;
        Ok(head)
    }

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

    pub fn publish_snap(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        scope: &str,
        gate: &str,
    ) -> Result<Publication> {
        self.publish_snap_with_resolution(store, snap, scope, gate, None)
    }

    pub fn publish_snap_metadata_only(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        scope: &str,
        gate: &str,
    ) -> Result<Publication> {
        self.publish_snap_inner(store, snap, scope, gate, None, true)
    }

    pub fn publish_snap_with_resolution(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        scope: &str,
        gate: &str,
        resolution: Option<PublicationResolution>,
    ) -> Result<Publication> {
        self.publish_snap_inner(store, snap, scope, gate, resolution, false)
    }

    pub fn upload_snap_objects(&self, store: &LocalStore, snap: &SnapRecord) -> Result<()> {
        // Reuse the publish upload path but skip publication creation.
        let (blobs, manifests, recipes) = collect_objects(store, &snap.root_manifest)?;
        let manifest_order = manifest_postorder(store, &snap.root_manifest)?;

        let repo = &self.remote.repo_id;
        let resp = with_retries("missing objects request", || {
            self.client
                .post(self.url(&format!("/repos/{}/objects/missing", repo)))
                .header(reqwest::header::AUTHORIZATION, self.auth())
                .json(&MissingObjectsRequest {
                    blobs: blobs.iter().cloned().collect(),
                    manifests: manifests.iter().cloned().collect(),
                    recipes: recipes.iter().cloned().collect(),
                    snaps: vec![snap.id.clone()],
                })
                .send()
                .context("send")
        })?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }
        let resp = self.ensure_ok(resp, "missing objects")?;
        let missing: MissingObjectsResponse = resp.json().context("parse missing objects")?;

        for id in missing.missing_blobs {
            let bytes = store.get_blob(&ObjectId(id.clone()))?;
            with_retries(&format!("upload blob {}", id), || {
                let resp = self
                    .client
                    .put(self.url(&format!("/repos/{}/objects/blobs/{}", repo, id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload blob")
            })?;
        }

        for id in missing.missing_recipes {
            let rid = ObjectId(id.clone());
            let bytes = store.get_recipe_bytes(&rid)?;
            with_retries(&format!("upload recipe {}", id), || {
                let resp = self
                    .client
                    .put(self.url(&format!("/repos/{}/objects/recipes/{}", repo, id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload recipe")
            })?;
        }

        let mut missing_manifests: HashSet<String> =
            missing.missing_manifests.into_iter().collect();
        for mid in manifest_order {
            let id = mid.as_str();
            if !missing_manifests.remove(id) {
                continue;
            }
            let bytes = store.get_manifest_bytes(&mid)?;
            with_retries(&format!("upload manifest {}", id), || {
                let resp = self
                    .client
                    .put(self.url(&format!("/repos/{}/objects/manifests/{}", repo, id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload manifest")
            })?;
        }
        if !missing_manifests.is_empty() {
            anyhow::bail!("missing manifest postorder invariant violated");
        }

        // Upload snap record last.
        if missing.missing_snaps.contains(&snap.id) {
            with_retries("upload snap", || {
                let resp = self
                    .client
                    .put(self.url(&format!("/repos/{}/objects/snaps/{}", repo, snap.id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .json(snap)
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload snap")
            })?;
        }

        Ok(())
    }

    pub fn sync_snap(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        lane_id: &str,
        client_id: Option<String>,
    ) -> Result<LaneHead> {
        self.upload_snap_objects(store, snap)?;
        self.update_lane_head_me(lane_id, &snap.id, client_id)
    }

    fn publish_snap_inner(
        &self,
        store: &LocalStore,
        snap: &SnapRecord,
        scope: &str,
        gate: &str,
        resolution: Option<PublicationResolution>,
        metadata_only: bool,
    ) -> Result<Publication> {
        let (blobs, manifests, recipes) = collect_objects(store, &snap.root_manifest)?;
        let manifest_order = manifest_postorder(store, &snap.root_manifest)?;

        let repo = &self.remote.repo_id;
        let resp = with_retries("missing objects request", || {
            self.client
                .post(self.url(&format!("/repos/{}/objects/missing", repo)))
                .header(reqwest::header::AUTHORIZATION, self.auth())
                .json(&MissingObjectsRequest {
                    blobs: blobs.iter().cloned().collect(),
                    manifests: manifests.iter().cloned().collect(),
                    recipes: recipes.iter().cloned().collect(),
                    snaps: vec![snap.id.clone()],
                })
                .send()
                .context("send")
        })?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let resp = self.ensure_ok(resp, "missing objects")?;
        let missing: MissingObjectsResponse = resp.json().context("parse missing objects")?;

        if !metadata_only {
            for id in missing.missing_blobs {
                let bytes = store.get_blob(&ObjectId(id.clone()))?;
                with_retries(&format!("upload blob {}", id), || {
                    let resp = self
                        .client
                        .put(self.url(&format!("/repos/{}/objects/blobs/{}", repo, id)))
                        .header(reqwest::header::AUTHORIZATION, self.auth())
                        .body(bytes.clone())
                        .send()
                        .context("send")?;
                    self.ensure_ok(resp, "upload blob")
                })?;
            }
        }

        for id in missing.missing_recipes {
            let rid = ObjectId(id.clone());
            let bytes = store.get_recipe_bytes(&rid)?;

            let path = if metadata_only {
                format!(
                    "/repos/{}/objects/recipes/{}?allow_missing_blobs=true",
                    repo, id
                )
            } else {
                format!("/repos/{}/objects/recipes/{}", repo, id)
            };
            with_retries(&format!("upload recipe {}", id), || {
                let resp = self
                    .client
                    .put(self.url(&path))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload recipe")
            })?;
        }

        let mut missing_manifests: HashSet<String> =
            missing.missing_manifests.into_iter().collect();
        for mid in manifest_order {
            let id = mid.as_str();
            if !missing_manifests.remove(id) {
                continue;
            }

            let bytes = store.get_manifest_bytes(&mid)?;

            let path = if metadata_only {
                format!(
                    "/repos/{}/objects/manifests/{}?allow_missing_blobs=true",
                    repo, id
                )
            } else {
                format!("/repos/{}/objects/manifests/{}", repo, id)
            };
            with_retries(&format!("upload manifest {}", id), || {
                let resp = self
                    .client
                    .put(self.url(&path))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload manifest")
            })?;
        }

        if !missing_manifests.is_empty() {
            anyhow::bail!(
                "missing manifest upload ordering bug (still missing: {})",
                missing_manifests.len()
            );
        }

        if !missing.missing_snaps.is_empty() {
            with_retries("upload snap", || {
                let resp = self
                    .client
                    .put(self.url(&format!("/repos/{}/objects/snaps/{}", repo, snap.id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .json(snap)
                    .send()
                    .context("send")?;
                self.ensure_ok(resp, "upload snap")
            })?;
        }

        let resp = with_retries("create publication", || {
            let resp = self
                .client
                .post(self.url(&format!("/repos/{}/publications", repo)))
                .header(reqwest::header::AUTHORIZATION, self.auth())
                .json(&CreatePublicationRequest {
                    snap_id: snap.id.clone(),
                    scope: scope.to_string(),
                    gate: gate.to_string(),
                    metadata_only,
                    resolution: resolution.clone(),
                })
                .send()
                .context("send")?;
            self.ensure_ok(resp, "create publication")
        })?;

        let pubrec: Publication = resp.json().context("parse publication")?;
        Ok(pubrec)
    }
}
