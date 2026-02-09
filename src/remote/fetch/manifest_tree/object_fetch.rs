use super::*;

pub(super) fn fetch_blob_if_missing(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    blob: &ObjectId,
) -> Result<()> {
    if store.has_blob(blob) {
        return Ok(());
    }
    let bytes = with_retries(&format!("fetch blob {}", blob.as_str()), || {
        let resp = remote
            .client
            .get(remote.url(&format!("/repos/{}/objects/blobs/{}", repo, blob.as_str())))
            .header(reqwest::header::AUTHORIZATION, remote.auth())
            .send()
            .context("send")?;
        remote
            .ensure_ok(resp, "fetch blob")?
            .bytes()
            .context("bytes")
    })?;

    let computed = blake3::hash(&bytes).to_hex().to_string();
    if computed != blob.as_str() {
        anyhow::bail!(
            "blob hash mismatch (expected {}, got {})",
            blob.as_str(),
            computed
        );
    }
    let id = store.put_blob(&bytes)?;
    if &id != blob {
        anyhow::bail!("unexpected blob id mismatch");
    }
    Ok(())
}

pub(super) fn fetch_recipe_and_chunks(
    store: &LocalStore,
    remote: &RemoteClient,
    repo: &str,
    recipe: &ObjectId,
) -> Result<()> {
    if !store.has_recipe(recipe) {
        let bytes = with_retries(&format!("fetch recipe {}", recipe.as_str()), || {
            let resp = remote
                .client
                .get(remote.url(&format!(
                    "/repos/{}/objects/recipes/{}",
                    repo,
                    recipe.as_str()
                )))
                .header(reqwest::header::AUTHORIZATION, remote.auth())
                .send()
                .context("send")?;
            remote
                .ensure_ok(resp, "fetch recipe")?
                .bytes()
                .context("bytes")
        })?;

        store.put_recipe_bytes(recipe, &bytes)?;
    }

    let r = store.get_recipe(recipe)?;
    for c in r.chunks {
        fetch_blob_if_missing(store, remote, repo, &c.blob)?;
    }
    Ok(())
}
