use std::collections::HashSet;

use anyhow::{Context, Result};

use crate::model::{ObjectId, SnapRecord};
use crate::store::LocalStore;

use super::{MissingObjectsResponse, RemoteClient, with_retries};

mod blobs;
mod manifests;
mod recipes;
mod snap;

pub(super) fn upload_missing_objects(
    client: &RemoteClient,
    store: &LocalStore,
    repo: &str,
    snap: &SnapRecord,
    manifest_order: &[ObjectId],
    missing: MissingObjectsResponse,
    metadata_only: bool,
) -> Result<()> {
    if !metadata_only {
        blobs::upload_blobs(client, store, repo, missing.missing_blobs)?;
    }

    recipes::upload_recipes(client, store, repo, missing.missing_recipes, metadata_only)?;

    manifests::upload_manifests(
        client,
        store,
        repo,
        manifest_order,
        missing.missing_manifests,
        metadata_only,
    )?;

    if !missing.missing_snaps.is_empty() {
        snap::upload_snap(client, repo, snap)?;
    }

    Ok(())
}
