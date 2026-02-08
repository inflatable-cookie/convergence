//! Remote fetch/read paths and recursive manifest/object retrieval helpers.

use super::*;

impl RemoteClient {
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

pub(super) fn collect_objects(
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

pub(super) fn manifest_postorder(store: &LocalStore, root: &ObjectId) -> Result<Vec<ObjectId>> {
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

pub(super) fn fetch_manifest_tree_inner(
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
