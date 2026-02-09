use super::*;

mod blobs;
mod manifests;
mod recipes;
mod snaps;

pub(super) fn upload_blobs(
    client: &RemoteClient,
    store: &LocalStore,
    missing_blobs: &[String],
) -> Result<()> {
    blobs::upload_blobs(client, store, missing_blobs)
}

pub(super) fn upload_recipes(
    client: &RemoteClient,
    store: &LocalStore,
    missing_recipes: &[String],
) -> Result<()> {
    recipes::upload_recipes(client, store, missing_recipes)
}

pub(super) fn upload_manifests(
    client: &RemoteClient,
    store: &LocalStore,
    manifest_order: Vec<ObjectId>,
    missing_manifests: Vec<String>,
) -> Result<()> {
    manifests::upload_manifests(client, store, manifest_order, missing_manifests)
}

pub(super) fn upload_snap_if_needed(
    client: &RemoteClient,
    snap: &SnapRecord,
    missing_snaps: &[String],
) -> Result<()> {
    snaps::upload_snap_if_needed(client, snap, missing_snaps)
}
