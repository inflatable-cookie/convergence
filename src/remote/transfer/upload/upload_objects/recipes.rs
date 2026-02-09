use anyhow::Context;

use super::*;
use crate::model::ObjectId;

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
