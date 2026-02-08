//! Remote upload/publish/sync transfer workflows.

use super::fetch::{collect_objects, manifest_postorder};
use super::*;

impl RemoteClient {
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
