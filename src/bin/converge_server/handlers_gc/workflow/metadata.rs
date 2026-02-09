use super::*;

pub(super) fn prune_repo_metadata(
    state: &AppState,
    repo: &mut Repo,
    retained: &RetainedRoots,
    prune_metadata: bool,
    dry_run: bool,
) -> Result<(), Response> {
    if !prune_metadata || dry_run {
        return Ok(());
    }

    repo.bundles
        .retain(|b| retained.keep_bundles.contains(&b.id));
    repo.pinned_bundles
        .retain(|bundle_id| retained.keep_bundles.contains(bundle_id));
    repo.releases
        .retain(|r| retained.keep_bundles.contains(&r.bundle_id));
    repo.publications
        .retain(|p| retained.keep_publications.contains(&p.id));
    repo.snaps = retained.keep_snaps.clone();
    persist_repo(state, repo).map_err(internal_error)?;
    Ok(())
}
