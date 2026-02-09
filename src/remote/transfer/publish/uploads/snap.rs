use super::*;

pub(super) fn upload_snap(client: &RemoteClient, repo: &str, snap: &SnapRecord) -> Result<()> {
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
    Ok(())
}
