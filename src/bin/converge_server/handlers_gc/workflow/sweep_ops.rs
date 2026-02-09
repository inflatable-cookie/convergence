use super::*;

pub(super) fn sweep_repo_objects(
    state: &AppState,
    repo_id: &str,
    repo: &Repo,
    retained: &RetainedRoots,
    q: &GcQuery,
) -> Result<SweepCounts, Response> {
    let objects_root = repo_data_dir(state, repo_id).join("objects");
    let (deleted_blobs, kept_blobs_count) = sweep::sweep_ids(
        &objects_root.join("blobs"),
        None,
        &retained.keep_blobs,
        q.dry_run,
    )?;
    let (deleted_manifests, kept_manifests_count) = sweep::sweep_ids(
        &objects_root.join("manifests"),
        Some("json"),
        &retained.keep_manifests,
        q.dry_run,
    )?;
    let (deleted_recipes, kept_recipes_count) = sweep::sweep_ids(
        &objects_root.join("recipes"),
        Some("json"),
        &retained.keep_recipes,
        q.dry_run,
    )?;

    let (deleted_snaps, _) = if q.prune_metadata {
        sweep::sweep_ids(
            &objects_root.join("snaps"),
            Some("json"),
            &retained.keep_snaps,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    let (deleted_bundles, _) = if q.prune_metadata {
        sweep::sweep_ids(
            &repo_data_dir(state, repo_id).join("bundles"),
            Some("json"),
            &retained.keep_bundles,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    let keep_release_ids: HashSet<String> = repo
        .releases
        .iter()
        .filter(|r| retained.keep_bundles.contains(&r.bundle_id))
        .map(|r| r.id.clone())
        .collect();

    let (deleted_releases, kept_releases_count) = if q.prune_metadata {
        sweep::sweep_ids(
            &repo_data_dir(state, repo_id).join("releases"),
            Some("json"),
            &keep_release_ids,
            q.dry_run,
        )?
    } else {
        (0, 0)
    };

    Ok(SweepCounts {
        deleted_blobs,
        kept_blobs_count,
        deleted_manifests,
        kept_manifests_count,
        deleted_recipes,
        kept_recipes_count,
        deleted_snaps,
        deleted_bundles,
        deleted_releases,
        kept_releases_count,
    })
}
