//! Garbage-collection endpoint logic for retained and prunable repo objects.

use super::*;

#[derive(Debug, serde::Deserialize)]
pub(super) struct GcQuery {
    #[serde(default = "default_true")]
    dry_run: bool,
    #[serde(default = "default_true")]
    prune_metadata: bool,

    /// If set, prune release history by keeping only the latest N releases per channel.
    ///
    /// This affects GC roots: pruned releases stop retaining their referenced bundles/objects.
    #[serde(default)]
    prune_releases_keep_last: Option<usize>,
}

pub(super) async fn gc_repo(
    State(state): State<Arc<AppState>>,
    Extension(subject): Extension<Subject>,
    Path(repo_id): Path<String>,
    Query(q): Query<GcQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    let mut repos = state.repos.write().await;
    let repo = repos.get_mut(&repo_id).ok_or_else(not_found)?;
    if !can_publish(repo, &subject) {
        return Err(forbidden());
    }

    if !q.prune_metadata && !q.dry_run {
        return Err(bad_request(anyhow::anyhow!(
            "refusing destructive GC with prune_metadata=false (would create dangling references); use dry_run=true or prune_metadata=true"
        )));
    }

    // Optional release history pruning.
    let releases_before = repo.releases.len();
    let mut pruned_releases_keep_last = 0usize;
    if let Some(keep_last) = q.prune_releases_keep_last {
        if keep_last == 0 {
            return Err(bad_request(anyhow::anyhow!(
                "prune_releases_keep_last must be >= 1"
            )));
        }

        let mut by_channel: HashMap<String, Vec<Release>> = HashMap::new();
        for r in repo.releases.clone() {
            by_channel.entry(r.channel.clone()).or_default().push(r);
        }

        let mut kept: Vec<Release> = Vec::new();
        for (_ch, mut rs) in by_channel {
            rs.sort_by(|a, b| b.released_at.cmp(&a.released_at));
            rs.truncate(keep_last);
            kept.extend(rs);
        }
        kept.sort_by(|a, b| b.released_at.cmp(&a.released_at));
        pruned_releases_keep_last = releases_before.saturating_sub(kept.len());
        repo.releases = kept;
    }

    // Retention roots: pinned bundles, releases, and current promotion-state pointers.
    let mut keep_bundles: HashSet<String> = repo.pinned_bundles.iter().cloned().collect();
    for r in &repo.releases {
        keep_bundles.insert(r.bundle_id.clone());
    }
    for per_scope in repo.promotion_state.values() {
        for bid in per_scope.values() {
            keep_bundles.insert(bid.clone());
        }
    }

    let mut keep_publications: HashSet<String> = HashSet::new();
    let mut keep_snaps: HashSet<String> = HashSet::new();
    let mut keep_blobs: HashSet<String> = HashSet::new();
    let mut keep_manifests: HashSet<String> = HashSet::new();
    let mut keep_recipes: HashSet<String> = HashSet::new();

    let mut bundle_roots: Vec<String> = Vec::new();
    for bid in &keep_bundles {
        let bundle = if let Some(b) = repo.bundles.iter().find(|b| b.id == *bid) {
            b.clone()
        } else {
            load_bundle_from_disk(state.as_ref(), &repo_id, bid)?
        };

        bundle_roots.push(bundle.root_manifest.clone());
        for pid in bundle.input_publications {
            keep_publications.insert(pid);
        }
    }

    for p in &repo.publications {
        if keep_publications.contains(&p.id) {
            keep_snaps.insert(p.snap_id.clone());
        }
    }

    // Lane heads are unpublished collaboration roots.
    for lane in repo.lanes.values() {
        for h in lane.heads.values() {
            keep_snaps.insert(h.snap_id.clone());
        }

        for hist in lane.head_history.values() {
            for h in hist {
                keep_snaps.insert(h.snap_id.clone());
            }
        }
    }

    // Collect objects from kept bundle roots.
    for root in &bundle_roots {
        collect_objects_from_manifest_tree(
            state.as_ref(),
            &repo_id,
            root,
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    // Collect objects from kept snaps (provenance roots).
    for sid in keep_snaps.clone() {
        let path = repo_data_dir(state.as_ref(), &repo_id)
            .join("objects/snaps")
            .join(format!("{}.json", sid));
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read {}", path.display()))
            .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        let snap: converge::model::SnapRecord =
            serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        collect_objects_from_manifest_tree(
            state.as_ref(),
            &repo_id,
            snap.root_manifest.as_str(),
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    fn sweep_ids(
        dir: &std::path::Path,
        ext: Option<&str>,
        keep: &HashSet<String>,
        dry_run: bool,
    ) -> Result<(usize, usize), Response> {
        if !dir.is_dir() {
            return Ok((0, 0));
        }
        let mut deleted = 0;
        let mut kept = 0;
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("read {}", dir.display()))
            .map_err(|e| internal_error(anyhow::anyhow!(e)))?
        {
            let entry = entry
                .with_context(|| format!("read {} entry", dir.display()))
                .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let id = match ext {
                None => path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string()),
                Some(e) => {
                    if path.extension().and_then(|s| s.to_str()) != Some(e) {
                        continue;
                    }
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                }
            };
            let Some(id) = id else {
                continue;
            };
            if id.len() != 64 {
                continue;
            }
            if keep.contains(&id) {
                kept += 1;
                continue;
            }
            deleted += 1;
            if !dry_run {
                std::fs::remove_file(&path)
                    .with_context(|| format!("remove {}", path.display()))
                    .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
            }
        }
        Ok((deleted, kept))
    }

    // Sweep objects.
    let objects_root = repo_data_dir(state.as_ref(), &repo_id).join("objects");
    let (deleted_blobs, kept_blobs_count) =
        sweep_ids(&objects_root.join("blobs"), None, &keep_blobs, q.dry_run)?;
    let (deleted_manifests, kept_manifests_count) = sweep_ids(
        &objects_root.join("manifests"),
        Some("json"),
        &keep_manifests,
        q.dry_run,
    )?;
    let (deleted_recipes, kept_recipes_count) = sweep_ids(
        &objects_root.join("recipes"),
        Some("json"),
        &keep_recipes,
        q.dry_run,
    )?;

    let (deleted_snaps, _kept_snaps_count) = if q.prune_metadata {
        sweep_ids(
            &objects_root.join("snaps"),
            Some("json"),
            &keep_snaps,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    let (deleted_bundles, _kept_bundles_count) = if q.prune_metadata {
        sweep_ids(
            &repo_data_dir(state.as_ref(), &repo_id).join("bundles"),
            Some("json"),
            &keep_bundles,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    // Sweep releases (metadata).
    let keep_release_ids: HashSet<String> = repo
        .releases
        .iter()
        .filter(|r| keep_bundles.contains(&r.bundle_id))
        .map(|r| r.id.clone())
        .collect();

    let (deleted_releases, kept_releases_count) = if q.prune_metadata {
        sweep_ids(
            &repo_data_dir(state.as_ref(), &repo_id).join("releases"),
            Some("json"),
            &keep_release_ids,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    if q.prune_metadata && !q.dry_run {
        repo.bundles.retain(|b| keep_bundles.contains(&b.id));
        repo.pinned_bundles.retain(|b| keep_bundles.contains(b));
        repo.releases
            .retain(|r| keep_bundles.contains(&r.bundle_id));
        repo.publications
            .retain(|p| keep_publications.contains(&p.id));
        repo.snaps = keep_snaps.clone();
        persist_repo(state.as_ref(), repo).map_err(internal_error)?;
    }

    Ok(Json(serde_json::json!({
        "dry_run": q.dry_run,
        "prune_metadata": q.prune_metadata,
        "pruned": {
            "releases_keep_last": pruned_releases_keep_last
        },
        "kept": {
            "bundles": keep_bundles.len(),
            "releases": kept_releases_count,
            "publications": keep_publications.len(),
            "snaps": keep_snaps.len(),
            "blobs": kept_blobs_count,
            "manifests": kept_manifests_count,
            "recipes": kept_recipes_count
        },
        "deleted": {
            "bundles": deleted_bundles,
            "releases": deleted_releases,
            "snaps": deleted_snaps,
            "blobs": deleted_blobs,
            "manifests": deleted_manifests,
            "recipes": deleted_recipes
        }
    })))
}
