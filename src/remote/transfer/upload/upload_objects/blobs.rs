use anyhow::Context;

use super::*;
use crate::model::ObjectId;

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
