use super::*;

pub(super) fn upload_blobs(
    client: &RemoteClient,
    store: &LocalStore,
    repo: &str,
    blob_ids: Vec<String>,
) -> Result<()> {
    for id in blob_ids {
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
