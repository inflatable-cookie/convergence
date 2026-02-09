use super::*;

mod bundle_refs;
mod snap_refs;

pub(super) struct RetainedRoots {
    pub(super) keep_bundles: HashSet<String>,
    pub(super) keep_publications: HashSet<String>,
    pub(super) keep_snaps: HashSet<String>,
    pub(super) keep_blobs: HashSet<String>,
    pub(super) keep_manifests: HashSet<String>,
    pub(super) keep_recipes: HashSet<String>,
}

pub(super) fn collect_retained_roots(
    state: &AppState,
    repo_id: &str,
    repo: &Repo,
) -> Result<RetainedRoots, Response> {
    let keep_bundles = bundle_refs::collect_kept_bundle_ids(repo);
    let (bundle_roots, keep_publications) =
        bundle_refs::collect_bundle_roots_and_publications(state, repo_id, repo, &keep_bundles)?;

    let keep_snaps = snap_refs::collect_snap_ids(repo, &keep_publications);
    let (keep_blobs, keep_manifests, keep_recipes) =
        snap_refs::collect_tree_objects(state, repo_id, &bundle_roots, &keep_snaps)?;

    Ok(RetainedRoots {
        keep_bundles,
        keep_publications,
        keep_snaps,
        keep_blobs,
        keep_manifests,
        keep_recipes,
    })
}
