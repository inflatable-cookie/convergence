use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};

use crate::model::{ObjectId, RemoteConfig, SnapRecord};
use crate::store::LocalStore;

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MissingObjectsResponse {
    pub missing_blobs: Vec<String>,
    pub missing_manifests: Vec<String>,
    pub missing_recipes: Vec<String>,
    pub missing_snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Pins {
    pub bundles: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct MissingObjectsRequest {
    blobs: Vec<String>,
    manifests: Vec<String>,
    recipes: Vec<String>,
    snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,

    #[serde(default, skip_serializing_if = "is_false")]
    metadata_only: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    resolution: Option<PublicationResolution>,
}

#[derive(Debug, serde::Serialize)]
struct CreateRepoRequest {
    id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Repo {
    pub id: String,
    pub owner: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RepoMembers {
    pub owner: String,
    pub readers: Vec<String>,
    pub publishers: Vec<String>,

    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub reader_user_ids: Vec<String>,
    #[serde(default)]
    pub publisher_user_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LaneMembers {
    pub lane: String,
    pub members: Vec<String>,

    #[serde(default)]
    pub member_user_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Publication {
    pub id: String,
    pub snap_id: String,
    pub scope: String,
    pub gate: String,
    pub publisher: String,
    pub created_at: String,

    #[serde(default)]
    pub resolution: Option<PublicationResolution>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PublicationResolution {
    pub bundle_id: String,
    pub root_manifest: String,
    pub resolved_root_manifest: String,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Bundle {
    pub id: String,
    pub scope: String,
    pub gate: String,
    pub root_manifest: String,
    pub input_publications: Vec<String>,
    pub created_by: String,
    pub created_at: String,
    pub promotable: bool,
    pub reasons: Vec<String>,

    #[serde(default)]
    pub approvals: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Promotion {
    pub id: String,
    pub bundle_id: String,
    pub scope: String,
    pub from_gate: String,
    pub to_gate: String,
    pub promoted_by: String,
    pub promoted_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WhoAmI {
    pub user: String,
    pub user_id: String,
    pub admin: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TokenView {
    pub id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateTokenResponse {
    pub id: String,
    pub token: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RemoteUser {
    pub id: String,
    pub handle: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub admin: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaneHead {
    pub snap_id: String,
    pub updated_at: String,

    #[serde(default)]
    pub client_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lane {
    pub id: String,
    pub members: HashSet<String>,

    #[serde(default)]
    pub heads: HashMap<String, LaneHead>,
}

#[derive(Debug, serde::Serialize)]
struct UpdateLaneHeadRequest {
    snap_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GateGraph {
    pub version: u32,
    pub terminal_gate: String,
    pub gates: Vec<GateDef>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GateDef {
    pub id: String,
    pub name: String,
    pub upstream: Vec<String>,

    #[serde(default)]
    pub allow_superpositions: bool,

    #[serde(default)]
    pub allow_metadata_only_publications: bool,

    #[serde(default)]
    pub required_approvals: u32,
}

pub struct RemoteClient {
    remote: RemoteConfig,
    token: String,
    client: reqwest::blocking::Client,
}

fn with_retries<T>(label: &str, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    const ATTEMPTS: usize = 3;
    let mut last: Option<anyhow::Error> = None;
    for i in 0..ATTEMPTS {
        match f() {
            Ok(v) => return Ok(v),
            Err(err) => {
                last = Some(err);
                if i + 1 < ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(200 * (1 << i)));
                }
            }
        }
    }
    Err(last
        .unwrap_or_else(|| anyhow::anyhow!("unknown error"))
        .context(label.to_string()))
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

    fn ensure_ok(
        &self,
        resp: reqwest::blocking::Response,
        label: &str,
    ) -> Result<reqwest::blocking::Response> {
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!(
                "unauthorized (token invalid/expired; run `converge login --url ... --token ... --repo ...` or `converge remote set --url ... --token ... --repo ...`)"
            );
        }
        resp.error_for_status()
            .with_context(|| format!("{} status", label))
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

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.remote.base_url, path)
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

    pub fn fetch_publications(
        &self,
        store: &LocalStore,
        only_snap: Option<&str>,
    ) -> Result<Vec<String>> {
        let repo = &self.remote.repo_id;
        let pubs = self.list_publications()?;
        let pubs = pubs
            .into_iter()
            .filter(|p| only_snap.map(|s| p.snap_id == s).unwrap_or(true))
            .collect::<Vec<_>>();

        let mut fetched = Vec::new();
        for p in pubs {
            if let Some(id) = self.fetch_snap_by_id(store, repo, &p.snap_id)? {
                fetched.push(id);
            }
        }

        Ok(fetched)
    }

    pub fn fetch_manifest_tree(&self, store: &LocalStore, root_manifest: &ObjectId) -> Result<()> {
        let repo = &self.remote.repo_id;
        fetch_manifest_tree(store, self, repo, root_manifest)
    }

    pub fn fetch_lane_heads(
        &self,
        store: &LocalStore,
        lane_id: &str,
        user: Option<&str>,
    ) -> Result<Vec<String>> {
        let repo = &self.remote.repo_id;

        let snap_ids: Vec<String> = if let Some(user) = user {
            vec![self.get_lane_head(lane_id, user)?.snap_id]
        } else {
            let lanes = self.list_lanes()?;
            let lane = lanes
                .into_iter()
                .find(|l| l.id == lane_id)
                .with_context(|| format!("lane not found: {}", lane_id))?;
            lane.heads.values().map(|h| h.snap_id.clone()).collect()
        };

        let mut fetched = Vec::new();
        for sid in snap_ids {
            if let Some(id) = self.fetch_snap_by_id(store, repo, &sid)? {
                fetched.push(id);
            }
        }
        Ok(fetched)
    }

    fn fetch_snap_by_id(
        &self,
        store: &LocalStore,
        repo: &str,
        snap_id: &str,
    ) -> Result<Option<String>> {
        if store.has_snap(snap_id) {
            return Ok(None);
        }

        let snap_bytes = with_retries(&format!("fetch snap {}", snap_id), || {
            let resp = self
                .client
                .get(self.url(&format!("/repos/{}/objects/snaps/{}", repo, snap_id)))
                .header(reqwest::header::AUTHORIZATION, self.auth())
                .send()
                .context("send")?;
            self.ensure_ok(resp, "fetch snap")?.bytes().context("bytes")
        })?;

        let snap: SnapRecord = serde_json::from_slice(&snap_bytes).context("parse snap")?;
        store.put_snap(&snap)?;

        fetch_manifest_tree(store, self, repo, &ObjectId(snap.root_manifest.0.clone()))?;
        Ok(Some(snap.id))
    }
}

fn collect_objects(
    store: &LocalStore,
    root: &ObjectId,
) -> Result<(HashSet<String>, HashSet<String>, HashSet<String>)> {
    let mut blobs = HashSet::new();
    let mut manifests = HashSet::new();
    let mut recipes = HashSet::new();
    let mut stack = vec![root.clone()];

    while let Some(mid) = stack.pop() {
        if !manifests.insert(mid.as_str().to_string()) {
            continue;
        }
        let m = store.get_manifest(&mid)?;
        for e in m.entries {
            match e.kind {
                crate::model::ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                crate::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                    recipes.insert(recipe.as_str().to_string());
                    let r = store.get_recipe(&recipe)?;
                    for c in r.chunks {
                        blobs.insert(c.blob.as_str().to_string());
                    }
                }
                crate::model::ManifestEntryKind::Dir { manifest } => {
                    stack.push(manifest);
                }
                crate::model::ManifestEntryKind::Symlink { .. } => {}
                crate::model::ManifestEntryKind::Superposition { .. } => {
                    anyhow::bail!("cannot publish snap containing superpositions");
                }
            }
        }
    }

    Ok((blobs, manifests, recipes))
}

fn manifest_postorder(store: &LocalStore, root: &ObjectId) -> Result<Vec<ObjectId>> {
    fn visit(
        store: &LocalStore,
        id: &ObjectId,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        out: &mut Vec<ObjectId>,
    ) -> Result<()> {
        let key = id.as_str().to_string();
        if visited.contains(&key) {
            return Ok(());
        }
        if !visiting.insert(key.clone()) {
            anyhow::bail!("cycle detected in manifest graph at {}", id.as_str());
        }

        let manifest = store.get_manifest(id)?;
        for e in manifest.entries {
            if let crate::model::ManifestEntryKind::Dir { manifest } = e.kind {
                visit(store, &manifest, visiting, visited, out)?;
            }
        }

        visiting.remove(&key);
        visited.insert(key);
        out.push(id.clone());
        Ok(())
    }

    let mut out = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    visit(store, root, &mut visiting, &mut visited, &mut out)?;
    Ok(out)
}

fn fetch_manifest_tree(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    root: &ObjectId,
) -> Result<()> {
    let mut visited = HashSet::new();
    fetch_manifest_tree_inner(store, remote, repo, root, &mut visited)
}

fn fetch_manifest_tree_inner(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    manifest_id: &ObjectId,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if !visited.insert(manifest_id.as_str().to_string()) {
        return Ok(());
    }

    if !store.has_manifest(manifest_id) {
        let resp = remote
            .client
            .get(remote.url(&format!(
                "/repos/{}/objects/manifests/{}",
                repo,
                manifest_id.as_str()
            )))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("fetch manifest")?;
        let bytes = remote
            .ensure_ok(resp, "fetch manifest")?
            .bytes()
            .context("read manifest bytes")?;

        store.put_manifest_bytes(manifest_id, &bytes)?;
    }

    let manifest = store.get_manifest(manifest_id)?;
    for e in manifest.entries {
        match e.kind {
            crate::model::ManifestEntryKind::Dir { manifest } => {
                fetch_manifest_tree_inner(store, remote, repo, &manifest, visited)?;
            }
            crate::model::ManifestEntryKind::File { blob, .. } => {
                fetch_blob_if_missing(store, remote, repo, &blob)?;
            }
            crate::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                fetch_recipe_and_chunks(store, remote, repo, &recipe)?;
            }
            crate::model::ManifestEntryKind::Symlink { .. } => {}
            crate::model::ManifestEntryKind::Superposition { variants } => {
                for v in variants {
                    match v.kind {
                        crate::model::SuperpositionVariantKind::File { blob, .. } => {
                            fetch_blob_if_missing(store, remote, repo, &blob)?;
                        }
                        crate::model::SuperpositionVariantKind::Dir { manifest } => {
                            fetch_manifest_tree_inner(store, remote, repo, &manifest, visited)?;
                        }
                        crate::model::SuperpositionVariantKind::Symlink { .. } => {}
                        crate::model::SuperpositionVariantKind::Tombstone => {}
                        crate::model::SuperpositionVariantKind::FileChunks { recipe, .. } => {
                            fetch_recipe_and_chunks(store, remote, repo, &recipe)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn fetch_blob_if_missing(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    blob: &ObjectId,
) -> Result<()> {
    if store.has_blob(blob) {
        return Ok(());
    }
    let bytes = with_retries(&format!("fetch blob {}", blob.as_str()), || {
        let resp = remote
            .client
            .get(remote.url(&format!("/repos/{}/objects/blobs/{}", repo, blob.as_str())))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("send")?;
        remote
            .ensure_ok(resp, "fetch blob")?
            .bytes()
            .context("bytes")
    })?;

    let computed = blake3::hash(&bytes).to_hex().to_string();
    if computed != blob.as_str() {
        anyhow::bail!(
            "blob hash mismatch (expected {}, got {})",
            blob.as_str(),
            computed
        );
    }
    let id = store.put_blob(&bytes)?;
    if &id != blob {
        anyhow::bail!("unexpected blob id mismatch");
    }
    Ok(())
}

fn fetch_recipe_and_chunks(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    recipe: &ObjectId,
) -> Result<()> {
    if !store.has_recipe(recipe) {
        let bytes = with_retries(&format!("fetch recipe {}", recipe.as_str()), || {
            let resp = remote
                .client
                .get(remote.url(&format!(
                    "/repos/{}/objects/recipes/{}",
                    repo,
                    recipe.as_str()
                )))
                .header(reqwest::header::AUTHORIZATION, remote.auth())
                .send()
                .context("send")?;
            remote
                .ensure_ok(resp, "fetch recipe")?
                .bytes()
                .context("bytes")
        })?;

        store.put_recipe_bytes(recipe, &bytes)?;
    }

    let r = store.get_recipe(recipe)?;
    for c in r.chunks {
        fetch_blob_if_missing(store, remote, repo, &c.blob)?;
    }
    Ok(())
}
