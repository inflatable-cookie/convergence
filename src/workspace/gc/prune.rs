use anyhow::Result;

use super::GcReport;
use crate::store::LocalStore;

pub(super) fn prune_unreferenced_objects(
    store: &LocalStore,
    dry_run: bool,
    keep_blobs: &std::collections::HashSet<String>,
    keep_manifests: &std::collections::HashSet<String>,
    keep_recipes: &std::collections::HashSet<String>,
    report: &mut GcReport,
) -> Result<()> {
    for id in store.list_blob_ids()? {
        if !keep_blobs.contains(id.as_str()) {
            report.deleted_blobs += 1;
            if !dry_run {
                store.delete_blob(&id)?;
            }
        }
    }
    for id in store.list_manifest_ids()? {
        if !keep_manifests.contains(id.as_str()) {
            report.deleted_manifests += 1;
            if !dry_run {
                store.delete_manifest(&id)?;
            }
        }
    }
    for id in store.list_recipe_ids()? {
        if !keep_recipes.contains(id.as_str()) {
            report.deleted_recipes += 1;
            if !dry_run {
                store.delete_recipe(&id)?;
            }
        }
    }

    Ok(())
}
