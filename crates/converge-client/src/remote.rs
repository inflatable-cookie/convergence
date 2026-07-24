use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};

use crate::model::{
    AddLaneMemberRequest, ApproveRequest, BundleRecord, CreateLaneRequest, InboxReport, LaneRecord,
    Manifest, ManifestEntryKind, NegotiateRequest, NegotiateResponse, ObjectFrame, ObjectId,
    ObjectSet, PromoteRequest, PublishRequest, ReleaseRecord, ReleaseRequest, RetentionPolicy,
    SetLaneHeadRequest, SnapRecord, SuperpositionVariantKind, VerifyReport, WIRE_VERSION,
};
use crate::store::LocalStore;

/// Blocking sync client for the wire contract (arch 16). The TUI wraps this
/// behind its async task pool; the CLI calls it directly.
pub struct RemoteClient {
    base_url: String,
    token: String,
    http: reqwest::blocking::Client,
    /// Max bytes per transfer batch (doc 16 §1c); clients split above it.
    batch_cap: usize,
}

impl RemoteClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::blocking::Client::new(),
            batch_cap: 8 * 1024 * 1024,
        }
    }

    /// Test hook: shrink the batch cap to exercise splitting.
    pub fn with_batch_cap(mut self, cap: usize) -> Self {
        self.batch_cap = cap.max(1);
        self
    }

    /// Upload frames in cap-split batches (doc 16 §1c).
    fn put_frames(&self, frames: Vec<ObjectFrame>) -> Result<()> {
        let mut batch: Vec<ObjectFrame> = Vec::new();
        let mut batch_bytes = 0usize;
        let flush = |batch: &mut Vec<ObjectFrame>| -> Result<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let mut body = Vec::new();
            ciborium::into_writer(&batch, &mut body).context("encode batch")?;
            Self::check(
                self.http
                    .post(self.url("/api/objects/batch"))
                    .bearer_auth(&self.token)
                    .body(body)
                    .send()
                    .context("upload batch")?,
            )?;
            batch.clear();
            Ok(())
        };
        for frame in frames {
            if batch_bytes + frame.bytes.len() > self.batch_cap && !batch.is_empty() {
                flush(&mut batch)?;
                batch_bytes = 0;
            }
            batch_bytes += frame.bytes.len();
            batch.push(frame);
        }
        flush(&mut batch)
    }

    /// Download a set of objects as CBOR frames (single batch request;
    /// callers keep requests bounded by walking in waves).
    fn get_frames(&self, request: &ObjectSet) -> Result<Vec<ObjectFrame>> {
        if request.is_empty() {
            return Ok(Vec::new());
        }
        let response = Self::check(
            self.http
                .post(self.url("/api/objects/batch-get"))
                .bearer_auth(&self.token)
                .json(request)
                .send()
                .context("download batch")?,
        )?;
        let bytes = response.bytes().context("read batch body")?;
        ciborium::from_reader(bytes.as_ref()).context("decode batch")
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn check(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().unwrap_or_default();
        bail!("server returned {status}: {body}")
    }

    pub fn negotiate(&self, objects: ObjectSet) -> Result<ObjectSet> {
        let response = Self::check(
            self.http
                .post(self.url("/api/negotiate"))
                .bearer_auth(&self.token)
                .json(&NegotiateRequest {
                    wire_version: WIRE_VERSION,
                    objects,
                })
                .send()
                .context("negotiate")?,
        )?;
        let parsed: NegotiateResponse = response.json().context("parse negotiate response")?;
        Ok(parsed.missing)
    }

    /// Upload everything reachable from `root_manifest` that the server does
    /// not have. Negotiates manifests first and prunes blob/recipe collection
    /// to the subtrees the server is missing (Merkle prune).
    pub fn upload_tree(&self, store: &LocalStore, root_manifest: &ObjectId) -> Result<UploadStats> {
        let manifests = collect_manifests(store, root_manifest)?;
        let missing_manifests = self
            .negotiate(ObjectSet {
                manifests: manifests.to_vec(),
                ..Default::default()
            })?
            .manifests;

        // Only missing manifests' direct entries can name missing content.
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();
        for manifest_id in &missing_manifests {
            let manifest = store.get_manifest(manifest_id)?;
            collect_entry_objects(store, &manifest, &mut blobs, &mut recipes)?;
        }
        let missing = self.negotiate(ObjectSet {
            blobs: blobs.into_iter().collect(),
            recipes: recipes.into_iter().collect(),
            ..Default::default()
        })?;

        let mut frames: Vec<ObjectFrame> = Vec::new();
        for id in &missing.recipes {
            frames.push(ObjectFrame {
                kind: "recipes".into(),
                id: id.clone(),
                bytes: store.get_recipe_bytes(id)?,
            });
        }
        for id in &missing.blobs {
            frames.push(ObjectFrame {
                kind: "blobs".into(),
                id: id.clone(),
                bytes: store.get_blob(id)?,
            });
        }
        // Manifests last so a present root implies a complete subtree.
        for id in missing_manifests.iter().rev() {
            frames.push(ObjectFrame {
                kind: "manifests".into(),
                id: id.clone(),
                bytes: store.get_manifest_bytes(id)?,
            });
        }
        let uploaded = frames.len();
        self.put_frames(frames)?;
        Ok(UploadStats {
            negotiated_manifests: manifests.len(),
            uploaded,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &self,
        store: &LocalStore,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        snap: &SnapRecord,
        base_bundle_id: Option<String>,
        lane_id: Option<String>,
        notes: Option<String>,
    ) -> Result<(BundleRecord, UploadStats)> {
        let stats = self.upload_tree(store, &snap.root_manifest)?;
        let response = Self::check(
            self.http
                .post(self.url("/api/publish"))
                .bearer_auth(&self.token)
                .json(&PublishRequest {
                    wire_version: WIRE_VERSION,
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    gate_id: gate_id.into(),
                    snap: snap.clone(),
                    base_bundle_id,
                    lane_id,
                    notes,
                })
                .send()
                .context("publish")?,
        )?;
        let bundle: BundleRecord = response.json().context("parse publish response")?;
        Ok((bundle, stats))
    }

    pub fn get_bundle(&self, bundle_id: &str) -> Result<BundleRecord> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/bundles/{bundle_id}")))
                .bearer_auth(&self.token)
                .send()
                .context("get bundle")?,
        )?;
        response.json().context("parse bundle")
    }

    /// Download a bundle's tree into the local store; returns the root.
    pub fn fetch_bundle(&self, store: &LocalStore, bundle_id: &str) -> Result<ObjectId> {
        let bundle = self.get_bundle(bundle_id)?;
        let root = bundle
            .root_manifest
            .context("bundle has no root manifest")?;
        self.fetch_manifest_tree(store, &root)?;
        Ok(root)
    }

    /// Batched wave walk (doc 16 §1c): fetch manifests level by level,
    /// then their recipes, then all missing blobs in one request per wave.
    fn fetch_manifest_tree(&self, store: &LocalStore, manifest_id: &ObjectId) -> Result<()> {
        let mut manifest_wave: Vec<ObjectId> = vec![manifest_id.clone()];
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();

        while !manifest_wave.is_empty() {
            let need: Vec<ObjectId> = manifest_wave
                .iter()
                .filter(|id| !store.has_manifest(id))
                .cloned()
                .collect();
            for frame in self.get_frames(&ObjectSet {
                manifests: need,
                ..Default::default()
            })? {
                store.put_manifest_bytes(&frame.id, &frame.bytes)?;
            }
            let mut next = Vec::new();
            for id in &manifest_wave {
                let manifest = store.get_manifest(id)?;
                collect_kinds(&manifest, &mut blobs, &mut recipes, &mut next);
            }
            manifest_wave = next;
        }

        let need_recipes: Vec<ObjectId> = recipes
            .iter()
            .filter(|id| !store.has_recipe(id))
            .cloned()
            .collect();
        for frame in self.get_frames(&ObjectSet {
            recipes: need_recipes,
            ..Default::default()
        })? {
            store.put_recipe_bytes(&frame.id, &frame.bytes)?;
        }
        for id in &recipes {
            let recipe = store.get_recipe(id)?;
            for chunk in &recipe.chunks {
                blobs.insert(chunk.blob.clone());
            }
        }

        let need_blobs: Vec<ObjectId> = blobs
            .iter()
            .filter(|id| !store.has_blob(id))
            .cloned()
            .collect();
        for frame in self.get_frames(&ObjectSet {
            blobs: need_blobs,
            ..Default::default()
        })? {
            store.put_blob(&frame.bytes)?;
        }
        Ok(())
    }

    pub fn create_lane(
        &self,
        repo_id: &str,
        lane_id: &str,
        visibility: &str,
    ) -> Result<LaneRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/lanes")))
                .bearer_auth(&self.token)
                .json(&CreateLaneRequest {
                    lane_id: lane_id.into(),
                    visibility: visibility.into(),
                })
                .send()
                .context("create lane")?,
        )?;
        response.json().context("parse lane")
    }

    pub fn list_lanes(&self, repo_id: &str) -> Result<Vec<LaneRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/lanes")))
                .bearer_auth(&self.token)
                .send()
                .context("list lanes")?,
        )?;
        response.json().context("parse lanes")
    }

    pub fn add_lane_member(&self, repo_id: &str, lane_id: &str, member: &str) -> Result<()> {
        // Lane ids may contain '/'; encode the path segment.
        let lane_segment = lane_id.replace('%', "%25").replace('/', "%2F");
        Self::check(
            self.http
                .post(self.url(&format!(
                    "/api/repos/{repo_id}/lanes/{lane_segment}/members"
                )))
                .bearer_auth(&self.token)
                .json(&AddLaneMemberRequest {
                    member: member.into(),
                })
                .send()
                .context("add lane member")?,
        )?;
        Ok(())
    }

    /// Push the given snap's lineage to a lane head (unpublished sync):
    /// upload each lineage snap's tree + record (deepest first), then move
    /// the head. `lane_id: None` targets the personal lane.
    pub fn push_lineage(
        &self,
        store: &LocalStore,
        repo_id: &str,
        lane_id: Option<String>,
        head_snap_id: &str,
        force: bool,
    ) -> Result<crate::model::LaneHead> {
        // Collect the local lineage chain (skip thinned gaps).
        let mut chain = Vec::new();
        let mut stack = vec![head_snap_id.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) || !store.has_snap(&id) {
                continue;
            }
            let snap = store.get_snap(&id)?;
            stack.extend(snap.parents.iter().cloned());
            chain.push(snap);
        }
        // Deepest first so ancestry exists before descendants.
        for snap in chain.iter().rev() {
            self.upload_tree(store, &snap.root_manifest)?;
            Self::check(
                self.http
                    .put(self.url(&format!("/api/repos/{repo_id}/snaps/{}", snap.id)))
                    .bearer_auth(&self.token)
                    .json(snap)
                    .send()
                    .context("upload snap record")?,
            )?;
        }
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/lane-head")))
                .bearer_auth(&self.token)
                .json(&SetLaneHeadRequest {
                    lane_id,
                    snap_id: head_snap_id.into(),
                    force,
                })
                .send()
                .context("set lane head")?,
        )?;
        response.json().context("parse lane head")
    }

    /// Pull a lane head's lineage into the local store. No workspace
    /// mutation — restore stays an explicit act.
    pub fn pull_lane(&self, store: &LocalStore, repo_id: &str, lane_id: &str) -> Result<String> {
        let lane_segment = lane_id.replace('%', "%25").replace('/', "%2F");
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/lane-head/{lane_segment}")))
                .bearer_auth(&self.token)
                .send()
                .context("get lane head")?,
        )?;
        let head: crate::model::LaneHead = response.json().context("parse lane head")?;

        let mut stack = vec![head.snap_id.clone()];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) || store.has_snap(&id) {
                continue;
            }
            let response = Self::check(
                self.http
                    .get(self.url(&format!("/api/repos/{repo_id}/snaps/{id}")))
                    .bearer_auth(&self.token)
                    .send()
                    .context("get snap record")?,
            );
            // Thinned ancestors are absent server-side too: stop the walk
            // at gaps instead of failing the pull.
            let Ok(response) = response else { continue };
            let snap: SnapRecord = response.json().context("parse snap record")?;
            self.fetch_manifest_tree(store, &snap.root_manifest)?;
            stack.extend(snap.parents.iter().cloned());
            store.put_snap(&snap)?;
        }
        Ok(head.snap_id)
    }

    pub fn inbox(&self, repo_id: &str, scope_id: &str, since: Option<&str>) -> Result<InboxReport> {
        let mut request = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/inbox")))
            .query(&[("scope", scope_id)])
            .bearer_auth(&self.token);
        if let Some(since) = since {
            request = request.query(&[("since", since)]);
        }
        let response = Self::check(request.send().context("inbox")?)?;
        response.json().context("parse inbox")
    }

    pub fn verify(&self, bundle_id: &str) -> Result<VerifyReport> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/bundles/{bundle_id}/verify")))
                .bearer_auth(&self.token)
                .send()
                .context("verify")?,
        )?;
        response.json().context("parse verify report")
    }

    pub fn get_provenance(&self, bundle_id: &str) -> Result<crate::model::BundleProvenance> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/bundles/{bundle_id}/provenance")))
                .bearer_auth(&self.token)
                .send()
                .context("get provenance")?,
        )?;
        response.json().context("parse provenance")
    }

    pub fn approve(&self, bundle_id: &str, repo_id: &str, scope_id: &str) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/bundles/{bundle_id}/approve")))
                .bearer_auth(&self.token)
                .json(&ApproveRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                })
                .send()
                .context("approve")?,
        )?;
        Ok(())
    }

    pub fn release(
        &self,
        bundle_id: &str,
        repo_id: &str,
        scope_id: &str,
        channel: &str,
        notes: Option<String>,
    ) -> Result<ReleaseRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/bundles/{bundle_id}/release")))
                .bearer_auth(&self.token)
                .json(&ReleaseRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    channel: channel.into(),
                    notes,
                })
                .send()
                .context("release")?,
        )?;
        response.json().context("parse release")
    }

    pub fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/releases")))
                .bearer_auth(&self.token)
                .send()
                .context("list releases")?,
        )?;
        response.json().context("parse releases")
    }

    pub fn get_channel_head(&self, repo_id: &str, channel: &str) -> Result<ReleaseRecord> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/release/{channel}")))
                .bearer_auth(&self.token)
                .send()
                .context("channel head")?,
        )?;
        response.json().context("parse release")
    }

    pub fn gc(&self, repo_id: &str, dry_run: bool) -> Result<serde_json::Value> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/gc")))
                .query(&[("dry_run", if dry_run { "true" } else { "false" })])
                .bearer_auth(&self.token)
                .send()
                .context("gc")?,
        )?;
        response.json().context("parse gc report")
    }

    pub fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/retention")))
                .bearer_auth(&self.token)
                .send()
                .context("get retention")?,
        )?;
        response.json().context("parse retention")
    }

    pub fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()> {
        Self::check(
            self.http
                .put(self.url(&format!("/api/repos/{repo_id}/retention")))
                .bearer_auth(&self.token)
                .json(policy)
                .send()
                .context("set retention")?,
        )?;
        Ok(())
    }

    pub fn promote(
        &self,
        bundle_id: &str,
        repo_id: &str,
        scope_id: &str,
        to_gate: &str,
    ) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/bundles/{bundle_id}/promote")))
                .bearer_auth(&self.token)
                .json(&PromoteRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    to_gate: to_gate.into(),
                })
                .send()
                .context("promote")?,
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct UploadStats {
    pub negotiated_manifests: usize,
    pub uploaded: usize,
}

/// All manifest ids reachable from `root`, root first.
fn collect_manifests(store: &LocalStore, root: &ObjectId) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    let mut seen = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let manifest = store.get_manifest(&id)?;
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();
        let mut dirs = Vec::new();
        collect_kinds(&manifest, &mut blobs, &mut recipes, &mut dirs);
        stack.extend(dirs);
        out.push(id);
    }
    Ok(out)
}

/// Blobs and recipes named by one manifest's entries (recipes expand to
/// their chunk blobs via the local store).
fn collect_entry_objects(
    store: &LocalStore,
    manifest: &Manifest,
    blobs: &mut BTreeSet<ObjectId>,
    recipes: &mut BTreeSet<ObjectId>,
) -> Result<()> {
    let mut local_blobs = BTreeSet::new();
    let mut local_recipes = BTreeSet::new();
    let mut dirs = Vec::new();
    collect_kinds(manifest, &mut local_blobs, &mut local_recipes, &mut dirs);
    for recipe_id in &local_recipes {
        let recipe = store.get_recipe(recipe_id)?;
        for chunk in &recipe.chunks {
            blobs.insert(chunk.blob.clone());
        }
    }
    blobs.extend(local_blobs);
    recipes.extend(local_recipes);
    Ok(())
}

fn collect_kinds(
    manifest: &Manifest,
    blobs: &mut BTreeSet<ObjectId>,
    recipes: &mut BTreeSet<ObjectId>,
    dirs: &mut Vec<ObjectId>,
) {
    for entry in &manifest.entries {
        match &entry.kind {
            ManifestEntryKind::File { blob, .. } => {
                blobs.insert(blob.clone());
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                recipes.insert(recipe.clone());
            }
            ManifestEntryKind::Dir { manifest } => dirs.push(manifest.clone()),
            ManifestEntryKind::Symlink { .. } => {}
            ManifestEntryKind::Superposition { variants } => {
                for variant in variants {
                    match &variant.kind {
                        SuperpositionVariantKind::File { blob, .. } => {
                            blobs.insert(blob.clone());
                        }
                        SuperpositionVariantKind::FileChunks { recipe, .. } => {
                            recipes.insert(recipe.clone());
                        }
                        SuperpositionVariantKind::Dir { manifest } => dirs.push(manifest.clone()),
                        SuperpositionVariantKind::Symlink { .. }
                        | SuperpositionVariantKind::Tombstone => {}
                    }
                }
            }
        }
    }
}
