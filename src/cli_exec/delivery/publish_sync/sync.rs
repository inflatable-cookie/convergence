use super::*;

pub(in crate::cli_exec) fn handle_sync_command(
    ws: &Workspace,
    snap_id: Option<String>,
    lane: String,
    client_id: Option<String>,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    let snap = match snap_id {
        Some(id) => ws.show_snap(&id)?,
        None => ws
            .list_snaps()?
            .into_iter()
            .next()
            .context("no snaps to sync")?,
    };

    let head = client.sync_snap(&ws.store, &snap, &lane, client_id)?;

    ws.store
        .set_lane_sync(&lane, &snap.id, &head.updated_at)
        .context("record lane sync")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&head).context("serialize sync json")?
        );
    } else {
        println!("Synced {} to lane {}", snap.id, lane);
    }

    Ok(())
}
