use std::collections::HashSet;

use anyhow::{Context, Result};

use crate::model::SnapRecord;

use super::{MissingObjectsRequest, MissingObjectsResponse, RemoteClient, with_retries};

pub(super) fn query_missing_objects(
    client: &RemoteClient,
    snap: &SnapRecord,
    blobs: &HashSet<String>,
    manifests: &HashSet<String>,
    recipes: &HashSet<String>,
) -> Result<MissingObjectsResponse> {
    let repo = &client.remote.repo_id;
    let resp = with_retries("missing objects request", || {
        client
            .client
            .post(client.url(&format!("/repos/{}/objects/missing", repo)))
            .header(reqwest::header::AUTHORIZATION, client.auth())
            .json(&MissingObjectsRequest {
                blobs: blobs.iter().cloned().collect(),
                manifests: manifests.iter().cloned().collect(),
                recipes: recipes.iter().cloned().collect(),
                snaps: vec![snap.id.clone()],
            })
            .send()
            .context("send")
    })?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!(
            "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
        );
    }
    let resp = client.ensure_ok(resp, "missing objects")?;
    let missing: MissingObjectsResponse = resp.json().context("parse missing objects")?;
    Ok(missing)
}
