use super::*;

pub(super) fn upload_manifests(
    client: &RemoteClient,
    store: &LocalStore,
    repo: &str,
    manifest_order: &[ObjectId],
    missing_manifests: Vec<String>,
    metadata_only: bool,
) -> Result<()> {
    let mut missing_manifests: HashSet<String> = missing_manifests.into_iter().collect();
    for mid in manifest_order {
        let id = mid.as_str();
        if !missing_manifests.remove(id) {
            continue;
        }

        let bytes = store.get_manifest_bytes(mid)?;

        let path = if metadata_only {
            format!(
                "/repos/{}/objects/manifests/{}?allow_missing_blobs=true",
                repo, id
            )
        } else {
            format!("/repos/{}/objects/manifests/{}", repo, id)
        };
        with_retries(&format!("upload manifest {}", id), || {
            let resp = client
                .client
                .put(client.url(&path))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .body(bytes.clone())
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload manifest")
        })?;
    }

    if !missing_manifests.is_empty() {
        anyhow::bail!(
            "missing manifest upload ordering bug (still missing: {})",
            missing_manifests.len()
        );
    }

    Ok(())
}
