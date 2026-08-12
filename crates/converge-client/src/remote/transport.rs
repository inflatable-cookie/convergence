//! HTTP transport: request/response plumbing, negotiate, tree upload.

use anyhow::{Context, Result, bail};

use std::collections::BTreeSet;

use converge_model::{
    NegotiateRequest, NegotiateResponse, ObjectFrame, ObjectId, ObjectSet, WIRE_VERSION,
};

use crate::store::LocalStore;

use super::{
    MAX_BATCH_FRAMES, collect_entry_objects, collect_kinds, collect_manifests, split_object_set,
};

use super::{Progress, UploadStats};

use super::RemoteClient;

impl RemoteClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::blocking::Client::new(),
            batch_cap: 8 * 1024 * 1024,
            progress: None,
        }
    }

    /// Report transfer progress to `sink`. Off by default: a library
    /// that printed would be unusable from the TUI.
    pub fn with_progress(mut self, sink: std::sync::Arc<dyn Fn(Progress) + Send + Sync>) -> Self {
        self.progress = Some(sink);
        self
    }

    pub(crate) fn report(&self, progress: Progress) {
        if let Some(sink) = &self.progress {
            sink(progress);
        }
    }

    /// Test hook: shrink the batch cap to exercise splitting.
    pub fn with_batch_cap(mut self, cap: usize) -> Self {
        self.batch_cap = cap.max(1);
        self
    }

    /// Upload frames in cap-split batches (doc 16 §1c).
    fn put_frames(&self, repo_id: &str, frames: Vec<ObjectFrame>) -> Result<()> {
        let objects_total = frames.len();
        let bytes_total: u64 = frames.iter().map(|f| f.bytes.len() as u64).sum();
        let mut objects_done = 0usize;
        let mut bytes_done = 0u64;

        let mut batch: Vec<ObjectFrame> = Vec::new();
        let mut batch_bytes = 0usize;
        let flush = |batch: &mut Vec<ObjectFrame>,
                     objects_done: &mut usize,
                     bytes_done: &mut u64|
         -> Result<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let mut body = Vec::new();
            ciborium::into_writer(&batch, &mut body).context("encode batch")?;
            Self::check(
                self.http
                    .post(self.url(&format!("/api/repos/{repo_id}/objects/batch")))
                    .bearer_auth(&self.token)
                    .body(body)
                    .send()
                    .context("upload batch")?,
            )?;
            *objects_done += batch.len();
            *bytes_done += batch.iter().map(|f| f.bytes.len() as u64).sum::<u64>();
            self.report(Progress {
                phase: "upload",
                objects_done: *objects_done,
                objects_total,
                bytes_done: *bytes_done,
                bytes_total,
            });
            batch.clear();
            Ok(())
        };
        for frame in frames {
            if (batch_bytes + frame.bytes.len() > self.batch_cap || batch.len() >= MAX_BATCH_FRAMES)
                && !batch.is_empty()
            {
                flush(&mut batch, &mut objects_done, &mut bytes_done)?;
                batch_bytes = 0;
            }
            batch_bytes += frame.bytes.len();
            batch.push(frame);
        }
        flush(&mut batch, &mut objects_done, &mut bytes_done)
    }

    /// Download a set of objects as CBOR frames, splitting requests above
    /// the server's id cap (doc 16 §1c).
    fn get_frames(&self, repo_id: &str, request: &ObjectSet) -> Result<Vec<ObjectFrame>> {
        // Total is object count, not bytes: on the way down the sizes are
        // exactly what has not arrived yet.
        let objects_total = request.blobs.len() + request.recipes.len() + request.manifests.len();
        let mut bytes_done = 0u64;
        let mut frames = Vec::new();
        for chunk in split_object_set(request, MAX_BATCH_FRAMES) {
            let response = Self::check(
                self.http
                    .post(self.url(&format!("/api/repos/{repo_id}/objects/batch-get")))
                    .bearer_auth(&self.token)
                    .json(&chunk)
                    .send()
                    .context("download batch")?,
            )?;
            let bytes = response.bytes().context("read batch body")?;
            let mut decoded: Vec<ObjectFrame> =
                ciborium::from_reader(bytes.as_ref()).context("decode batch")?;
            bytes_done += decoded.iter().map(|f| f.bytes.len() as u64).sum::<u64>();
            frames.append(&mut decoded);
            self.report(Progress {
                phase: "download",
                objects_done: frames.len(),
                objects_total,
                bytes_done,
                bytes_total: bytes_done,
            });
        }
        Ok(frames)
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    pub(crate) fn check(
        response: reqwest::blocking::Response,
    ) -> Result<reqwest::blocking::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().unwrap_or_default();
        // The server answers errors as `{"error": "...", "ok": false}`.
        // Printing that envelope at a person makes them read JSON to find
        // the sentence inside it, and the sentence is the part that was
        // written for them — batch 26.3 watched a three-fault gate graph
        // refusal arrive wrapped in braces and quotes.
        //
        // Anything that is not that shape is passed through untouched: a
        // proxy's HTML or an empty body is still better than nothing.
        // The status stays in the chain rather than the headline: a
        // person needs the sentence, and callers that genuinely care
        // whether a refusal was 403 or 404 -- the secret routes answer
        // both deliberately, since existence is itself privileged --
        // still find it under `{err:#}`.
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = parsed.get("error").and_then(|e| e.as_str())
        {
            return Err(anyhow::anyhow!("http {status}")).context(message.to_string());
        }
        bail!("server returned {status}: {body}")
    }

    pub fn negotiate(&self, repo_id: &str, objects: ObjectSet) -> Result<ObjectSet> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/negotiate")))
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
    pub fn upload_tree(
        &self,
        store: &LocalStore,
        repo_id: &str,
        root_manifest: &ObjectId,
    ) -> Result<UploadStats> {
        let manifests = collect_manifests(store, root_manifest)?;
        let missing_set: BTreeSet<ObjectId> = self
            .negotiate(
                repo_id,
                ObjectSet {
                    manifests: manifests.to_vec(),
                    ..Default::default()
                },
            )?
            .manifests
            .into_iter()
            .collect();
        // Child-first: `collect_manifests` walks parent-first, so the
        // reverse never streams a parent before its children — a torn
        // batch stream cannot leave a parent without its subtree.
        let missing_manifests: Vec<ObjectId> = manifests
            .iter()
            .rev()
            .filter(|id| missing_set.contains(*id))
            .cloned()
            .collect();

        // Negotiate leaves from every reachable manifest, not only the
        // missing ones: a previously interrupted upload can have left
        // leaf holes under manifests the server already has.
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();
        for manifest_id in &manifests {
            let manifest = store.get_manifest(manifest_id)?;
            collect_entry_objects(store, &manifest, &mut blobs, &mut recipes)?;
        }
        let missing = self.negotiate(
            repo_id,
            ObjectSet {
                blobs: blobs.into_iter().collect(),
                recipes: recipes.into_iter().collect(),
                ..Default::default()
            },
        )?;

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
        for id in &missing_manifests {
            frames.push(ObjectFrame {
                kind: "manifests".into(),
                id: id.clone(),
                bytes: store.get_manifest_bytes(id)?,
            });
        }
        let uploaded = frames.len();
        self.put_frames(repo_id, frames)?;
        Ok(UploadStats {
            negotiated_manifests: manifests.len(),
            uploaded,
        })
    }

    /// Batched wave walk (doc 16 §1c): fetch manifests level by level,
    /// then their recipes, then all missing blobs in one request per wave.
    pub(crate) fn fetch_manifest_tree(
        &self,
        store: &LocalStore,
        repo_id: &str,
        manifest_id: &ObjectId,
    ) -> Result<()> {
        let mut manifest_wave: Vec<ObjectId> = vec![manifest_id.clone()];
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();

        while !manifest_wave.is_empty() {
            let need: Vec<ObjectId> = manifest_wave
                .iter()
                .filter(|id| !store.has_manifest(id))
                .cloned()
                .collect();
            for frame in self.get_frames(
                repo_id,
                &ObjectSet {
                    manifests: need,
                    ..Default::default()
                },
            )? {
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
        for frame in self.get_frames(
            repo_id,
            &ObjectSet {
                recipes: need_recipes,
                ..Default::default()
            },
        )? {
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
        for frame in self.get_frames(
            repo_id,
            &ObjectSet {
                blobs: need_blobs,
                ..Default::default()
            },
        )? {
            store.put_blob(&frame.bytes)?;
        }
        Ok(())
    }
}
