use super::*;

pub(super) fn collect_kept_bundle_ids(repo: &Repo) -> HashSet<String> {
    let mut keep_bundles: HashSet<String> = repo.pinned_bundles.iter().cloned().collect();
    for release in &repo.releases {
        keep_bundles.insert(release.bundle_id.clone());
    }
    for per_scope in repo.promotion_state.values() {
        for bundle_id in per_scope.values() {
            keep_bundles.insert(bundle_id.clone());
        }
    }
    keep_bundles
}

pub(super) fn collect_bundle_roots_and_publications(
    state: &AppState,
    repo_id: &str,
    repo: &Repo,
    keep_bundles: &HashSet<String>,
) -> Result<(Vec<String>, HashSet<String>), Response> {
    let mut keep_publications: HashSet<String> = HashSet::new();
    let mut bundle_roots: Vec<String> = Vec::new();
    for bundle_id in keep_bundles {
        let bundle = if let Some(existing) = repo.bundles.iter().find(|b| b.id == *bundle_id) {
            existing.clone()
        } else {
            load_bundle_from_disk(state, repo_id, bundle_id)?
        };

        bundle_roots.push(bundle.root_manifest.clone());
        for publication_id in bundle.input_publications {
            keep_publications.insert(publication_id);
        }
    }
    Ok((bundle_roots, keep_publications))
}
