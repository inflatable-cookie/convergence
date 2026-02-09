use std::collections::HashSet;

use anyhow::Context;

use super::*;
use crate::model::ObjectId;

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
