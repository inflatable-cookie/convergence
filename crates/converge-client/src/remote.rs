use std::collections::BTreeSet;

use anyhow::{Context, Result};

use crate::model::{Manifest, ManifestEntryKind, ObjectId, ObjectSet, SuperpositionVariantKind};
use crate::store::LocalStore;

/// Blocking sync client for the wire contract (arch 16). The TUI wraps this
/// behind its async task pool; the CLI calls it directly.
#[derive(Clone)]
pub struct RemoteClient {
    base_url: String,
    token: String,
    http: reqwest::blocking::Client,
    /// Max bytes per transfer batch (doc 16 §1c); clients split above it.
    batch_cap: usize,
    /// Optional transfer reporter (batch 16.4, audit P4.20).
    progress: Option<std::sync::Arc<dyn Fn(Progress) + Send + Sync>>,
}

impl std::fmt::Debug for RemoteClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteClient")
            .field("base_url", &self.base_url)
            .field("batch_cap", &self.batch_cap)
            .field("has_progress", &self.progress.is_some())
            .finish_non_exhaustive()
    }
}

/// Transfer progress, reported once per batch — the granularity the
/// wire actually moves in, and the one that matters for the beachhead's
/// large binaries (audit P4.20).
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    /// "upload" or "download".
    pub phase: &'static str,
    pub objects_done: usize,
    pub objects_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// What one probe of the server found (g02.022 batch 22.1).
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub reachable: bool,
    /// The credential was accepted. False on 401 — expired, revoked, or
    /// unknown, which the server's own message distinguishes.
    pub authenticated: bool,
    /// The credential is also allowed to do the thing. False on 403,
    /// which is a different problem from a bad token (batch 21.4).
    pub authorized: bool,
    pub status: Option<u16>,
    /// Server clock minus local clock. `None` when the server sent no
    /// usable `Date`.
    pub skew_seconds: Option<i64>,
    /// The server's own words, for the failing cases.
    pub detail: String,
}

impl RemoteClient {
    /// One page of a cursor listing (batch 15.2). `after` is the
    /// `next_cursor` of the previous page.
    fn page<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        after: Option<&str>,
        limit: Option<usize>,
        what: &'static str,
    ) -> Result<crate::model::Page<T>> {
        let mut request = self.http.get(self.url(path)).bearer_auth(&self.token);
        if let Some(after) = after {
            request = request.query(&[("after", after)]);
        }
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit.to_string())]);
        }
        let response = Self::check(request.send().context(what)?)?;
        response.json().context(what)
    }

    /// Follow a cursor listing to exhaustion. Pages are capped
    /// server-side, so this is a loop, not a single unbounded request.
    fn all_pages<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        what: &'static str,
    ) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page: crate::model::Page<T> = self.page(path, cursor.as_deref(), None, what)?;
            out.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(out),
            }
        }
    }
}

mod candidates;
mod identity;
mod lanes;
mod members;
mod secrets;
mod transport;

#[derive(Debug)]
pub struct UploadStats {
    pub negotiated_manifests: usize,
    pub uploaded: usize,
}

/// Server-side per-request id/frame cap (doc 16 §1c).
const MAX_BATCH_FRAMES: usize = 4096;

/// Split a request set into chunks the server accepts, preserving kinds.
fn split_object_set(request: &ObjectSet, cap: usize) -> Vec<ObjectSet> {
    let mut chunks: Vec<ObjectSet> = Vec::new();
    let mut current = ObjectSet::default();
    let mut count = 0usize;
    let push = |current: &mut ObjectSet, count: &mut usize, chunks: &mut Vec<ObjectSet>| {
        if !current.is_empty() {
            chunks.push(std::mem::take(current));
            *count = 0;
        }
    };
    for (kind, ids) in [
        (0, &request.blobs),
        (1, &request.manifests),
        (2, &request.recipes),
    ] {
        for id in ids {
            if count >= cap {
                push(&mut current, &mut count, &mut chunks);
            }
            match kind {
                0 => current.blobs.push(id.clone()),
                1 => current.manifests.push(id.clone()),
                _ => current.recipes.push(id.clone()),
            }
            count += 1;
        }
    }
    push(&mut current, &mut count, &mut chunks);
    chunks
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
