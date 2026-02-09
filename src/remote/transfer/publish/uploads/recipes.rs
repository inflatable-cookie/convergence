use super::*;

pub(super) fn upload_recipes(
    client: &RemoteClient,
    store: &LocalStore,
    repo: &str,
    recipe_ids: Vec<String>,
    metadata_only: bool,
) -> Result<()> {
    for id in recipe_ids {
        let rid = ObjectId(id.clone());
        let bytes = store.get_recipe_bytes(&rid)?;

        let path = if metadata_only {
            format!(
                "/repos/{}/objects/recipes/{}?allow_missing_blobs=true",
                repo, id
            )
        } else {
            format!("/repos/{}/objects/recipes/{}", repo, id)
        };
        with_retries(&format!("upload recipe {}", id), || {
            let resp = client
                .client
                .put(client.url(&path))
                .header(reqwest::header::AUTHORIZATION, client.auth())
                .body(bytes.clone())
                .send()
                .context("send")?;
            client.ensure_ok(resp, "upload recipe")
        })?;
    }
    Ok(())
}
