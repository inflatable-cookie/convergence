use super::*;

pub(super) fn gc_report(
    dry_run: bool,
    prune_metadata: bool,
    pruned_releases_keep_last: usize,
    retained: &RetainedRoots,
    counts: &SweepCounts,
) -> serde_json::Value {
    serde_json::json!({
        "dry_run": dry_run,
        "prune_metadata": prune_metadata,
        "pruned": {
            "releases_keep_last": pruned_releases_keep_last
        },
        "kept": {
            "bundles": retained.keep_bundles.len(),
            "releases": counts.kept_releases_count,
            "publications": retained.keep_publications.len(),
            "snaps": retained.keep_snaps.len(),
            "blobs": counts.kept_blobs_count,
            "manifests": counts.kept_manifests_count,
            "recipes": counts.kept_recipes_count
        },
        "deleted": {
            "bundles": counts.deleted_bundles,
            "releases": counts.deleted_releases,
            "snaps": counts.deleted_snaps,
            "blobs": counts.deleted_blobs,
            "manifests": counts.deleted_manifests,
            "recipes": counts.deleted_recipes
        }
    })
}
