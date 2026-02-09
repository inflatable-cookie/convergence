use std::collections::HashSet;

use anyhow::{Context, Result};

use crate::model::{ObjectId, SnapRecord};
use crate::store::LocalStore;

use super::{RemoteClient, with_retries};

pub(super) fn upload_blobs(
    client: &RemoteClient,
    store: &LocalStore,
    missing_blobs: &[String],
) -> Result<()> {
    let repo = &client.remote.repo_id;
    for id in missing_blobs {
        let bytes = store.get_blob(&ObjectId(id.clone()))?;
        with_retries(&format!("upload blob {}", id), || {
            let resp = client
                .client
                .put(client.url(&format!("/repos/{}/objects/blobs/{}", repo, id)))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .body(bytes.clone())
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload blob")
        })?;
    }
    Ok(())
}

pub(super) fn upload_recipes(
    client: &RemoteClient,
    store: &LocalStore,
    missing_recipes: &[String],
) -> Result<()> {
    let repo = &client.remote.repo_id;
    for id in missing_recipes {
        let rid = ObjectId(id.clone());
        let bytes = store.get_recipe_bytes(&rid)?;
        with_retries(&format!("upload recipe {}", id), || {
            let resp = client
                .client
                .put(client.url(&format!("/repos/{}/objects/recipes/{}", repo, id)))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .body(bytes.clone())
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload recipe")
        })?;
    }
    Ok(())
}

pub(super) fn upload_manifests(
    client: &RemoteClient,
    store: &LocalStore,
    manifest_order: Vec<ObjectId>,
    missing_manifests: Vec<String>,
) -> Result<()> {
    let repo = &client.remote.repo_id;
    let mut missing_manifests: HashSet<String> = missing_manifests.into_iter().collect();
    for mid in manifest_order {
        let id = mid.as_str();
        if !missing_manifests.remove(id) {
            continue;
        }
        let bytes = store.get_manifest_bytes(&mid)?;
        with_retries(&format!("upload manifest {}", id), || {
            let resp = client
                .client
                .put(client.url(&format!("/repos/{}/objects/manifests/{}", repo, id)))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .body(bytes.clone())
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload manifest")
        })?;
    }
    if !missing_manifests.is_empty() {
        anyhow::bail!("missing manifest postorder invariant violated");
    }
    Ok(())
}

pub(super) fn upload_snap_if_needed(
    client: &RemoteClient,
    snap: &SnapRecord,
    missing_snaps: &[String],
) -> Result<()> {
    if missing_snaps.contains(&snap.id) {
        let repo = &client.remote.repo_id;
        with_retries("upload snap", || {
            let resp = client
                .client
                .put(client.url(&format!("/repos/{}/objects/snaps/{}", repo, snap.id)))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .json(snap)
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload snap")
        })?;
    }
    Ok(())
}
