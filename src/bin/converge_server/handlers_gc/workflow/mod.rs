use super::roots::RetainedRoots;
use super::*;

mod metadata;
mod report;
mod sweep_ops;

#[derive(Debug)]
pub(super) struct SweepCounts {
    pub(super) deleted_blobs: usize,
    pub(super) kept_blobs_count: usize,
    pub(super) deleted_manifests: usize,
    pub(super) kept_manifests_count: usize,
    pub(super) deleted_recipes: usize,
    pub(super) kept_recipes_count: usize,
    pub(super) deleted_snaps: usize,
    pub(super) deleted_bundles: usize,
    pub(super) deleted_releases: usize,
    pub(super) kept_releases_count: usize,
}

pub(super) fn run_gc(
    state: &AppState,
    repo_id: &str,
    repo: &mut Repo,
    q: GcQuery,
) -> Result<serde_json::Value, Response> {
    let pruned_releases_keep_last = prune_release_history(repo, q.prune_releases_keep_last)?;
    let retained = collect_retained_roots(state, repo_id, repo)?;
    let counts = sweep_ops::sweep_repo_objects(state, repo_id, repo, &retained, &q)?;

    metadata::prune_repo_metadata(state, repo, &retained, q.prune_metadata, q.dry_run)?;

    Ok(report::gc_report(
        q.dry_run,
        q.prune_metadata,
        pruned_releases_keep_last,
        &retained,
        &counts,
    ))
}
