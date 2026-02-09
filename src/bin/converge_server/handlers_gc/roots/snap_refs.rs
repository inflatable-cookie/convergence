use super::*;

type ObjectSets = (HashSet<String>, HashSet<String>, HashSet<String>);

pub(super) fn collect_snap_ids(
    repo: &Repo,
    keep_publications: &HashSet<String>,
) -> HashSet<String> {
    let mut keep_snaps: HashSet<String> = HashSet::new();

    for publication in &repo.publications {
        if keep_publications.contains(&publication.id) {
            keep_snaps.insert(publication.snap_id.clone());
        }
    }

    for lane in repo.lanes.values() {
        for head in lane.heads.values() {
            keep_snaps.insert(head.snap_id.clone());
        }
        for history in lane.head_history.values() {
            for head in history {
                keep_snaps.insert(head.snap_id.clone());
            }
        }
    }

    keep_snaps
}

pub(super) fn collect_tree_objects(
    state: &AppState,
    repo_id: &str,
    bundle_roots: &[String],
    keep_snaps: &HashSet<String>,
) -> Result<ObjectSets, Response> {
    let mut keep_blobs: HashSet<String> = HashSet::new();
    let mut keep_manifests: HashSet<String> = HashSet::new();
    let mut keep_recipes: HashSet<String> = HashSet::new();

    for root_manifest in bundle_roots {
        collect_objects_from_manifest_tree(
            state,
            repo_id,
            root_manifest,
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    for snap_id in keep_snaps {
        let path = repo_data_dir(state, repo_id)
            .join("objects/snaps")
            .join(format!("{}.json", snap_id));
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read {}", path.display()))
            .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        let snap: converge::model::SnapRecord =
            serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
        collect_objects_from_manifest_tree(
            state,
            repo_id,
            snap.root_manifest.as_str(),
            &mut keep_blobs,
            &mut keep_manifests,
            &mut keep_recipes,
        )?;
    }

    Ok((keep_blobs, keep_manifests, keep_recipes))
}
