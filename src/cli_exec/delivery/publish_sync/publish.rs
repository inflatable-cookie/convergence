use super::*;

pub(in crate::cli_exec) fn handle_publish_command(
    ws: &Workspace,
    snap_id: Option<String>,
    scope: Option<String>,
    gate: Option<String>,
    metadata_only: bool,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote.clone(), token)?;

    let snap = match snap_id {
        Some(id) => ws.show_snap(&id)?,
        None => ws
            .list_snaps()?
            .into_iter()
            .next()
            .context("no snaps found (run `converge snap`)")?,
    };

    let scope = scope.unwrap_or_else(|| remote.scope.clone());
    let gate = gate.unwrap_or_else(|| remote.gate.clone());

    let pubrec = if metadata_only {
        client.publish_snap_metadata_only(&ws.store, &snap, &scope, &gate)?
    } else {
        client.publish_snap(&ws.store, &snap, &scope, &gate)?
    };

    ws.store
        .set_last_published(&remote, &scope, &gate, &snap.id)
        .context("record last published snap")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&pubrec).context("serialize publish json")?
        );
    } else {
        println!("Published {}", snap.id);
    }

    Ok(())
}
