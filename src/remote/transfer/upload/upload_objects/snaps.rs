use anyhow::Context;

use super::*;

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
