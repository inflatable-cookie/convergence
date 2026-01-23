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
    pub fn new(remote: RemoteConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("converge")
            .build()
            .context("build reqwest client")?;
        Ok(Self { remote, client })
    }

    pub fn remote(&self) -> &RemoteConfig {
        &self.remote
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.remote.token)
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

        let resp = resp.error_for_status().context("create repo status")?;
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

        let pubs: Vec<Publication> = resp
            .error_for_status()
            .context("list publications status")?
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

        let graph: GateGraph = resp
            .error_for_status()
            .context("get gate graph status")?
            .json()
            .context("parse gate graph")?;
        Ok(graph)
    }

    pub fn put_gate_graph(&self, graph: &GateGraph) -> Result<GateGraph> {
        let repo = &self.remote.repo_id;
        let graph: GateGraph = self
            .client
            .put(self.url(&format!("/repos/{}/gate-graph", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(graph)
            .send()
            .context("put gate graph")?
            .error_for_status()
            .context("put gate graph status")?
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
            .context("create bundle request")?
            .error_for_status()
            .context("create bundle status")?;
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

        let bundles: Vec<Bundle> = resp
            .error_for_status()
            .context("list bundles status")?
            .json()
            .context("parse bundles")?;
        Ok(bundles)
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

        let bundle: Bundle = resp
            .error_for_status()
            .context("get bundle status")?
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
            .context("promote request")?
            .error_for_status()
            .context("promote status")?;
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

        let resp = resp.error_for_status().context("promotion state status")?;

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
            .context("approve request")?
            .error_for_status()
            .context("approve status")?;

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

        let resp = resp.error_for_status().context("missing objects status")?;
        let missing: MissingObjectsResponse = resp.json().context("parse missing objects")?;

        if !metadata_only {
            for id in missing.missing_blobs {
                let bytes = store.get_blob(&ObjectId(id.clone()))?;
                with_retries(&format!("upload blob {}", id), || {
                    self.client
                        .put(self.url(&format!("/repos/{}/objects/blobs/{}", repo, id)))
                        .header(reqwest::header::AUTHORIZATION, self.auth())
                        .body(bytes.clone())
                        .send()
                        .context("send")?
                        .error_for_status()
                        .context("status")
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
                self.client
                    .put(self.url(&path))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?
                    .error_for_status()
                    .context("status")
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
                self.client
                    .put(self.url(&path))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .body(bytes.clone())
                    .send()
                    .context("send")?
                    .error_for_status()
                    .context("status")
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
                self.client
                    .put(self.url(&format!("/repos/{}/objects/snaps/{}", repo, snap.id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .json(snap)
                    .send()
                    .context("send")?
                    .error_for_status()
                    .context("status")
            })?;
        }

        let resp = with_retries("create publication", || {
            self.client
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
                .context("send")?
                .error_for_status()
                .context("status")
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
            if store.has_snap(&p.snap_id) {
                continue;
            }

            let snap_bytes = with_retries(&format!("fetch snap {}", p.snap_id), || {
                self.client
                    .get(self.url(&format!("/repos/{}/objects/snaps/{}", repo, p.snap_id)))
                    .header(reqwest::header::AUTHORIZATION, self.auth())
                    .send()
                    .context("send")?
                    .error_for_status()
                    .context("status")?
                    .bytes()
                    .context("bytes")
            })?;

            let snap: SnapRecord = serde_json::from_slice(&snap_bytes).context("parse snap")?;
            store.put_snap(&snap)?;

            fetch_manifest_tree(store, self, repo, &ObjectId(snap.root_manifest.0.clone()))?;
            fetched.push(snap.id);
        }

        Ok(fetched)
    }

    pub fn fetch_manifest_tree(&self, store: &LocalStore, root_manifest: &ObjectId) -> Result<()> {
        let repo = &self.remote.repo_id;
        fetch_manifest_tree(store, self, repo, root_manifest)
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
        let bytes = remote
            .client
            .get(remote.url(&format!(
                "/repos/{}/objects/manifests/{}",
                repo,
                manifest_id.as_str()
            )))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("fetch manifest")?
            .error_for_status()
            .context("fetch manifest status")?
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
        remote
            .client
            .get(remote.url(&format!("/repos/{}/objects/blobs/{}", repo, blob.as_str())))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("send")?
            .error_for_status()
            .context("status")?
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
            remote
                .client
                .get(remote.url(&format!(
                    "/repos/{}/objects/recipes/{}",
                    repo,
                    recipe.as_str()
                )))
                .header(reqwest::header::AUTHORIZATION, remote.auth())
                .send()
                .context("send")?
                .error_for_status()
                .context("status")?
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
