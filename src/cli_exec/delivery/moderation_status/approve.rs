use super::super::*;

pub(super) fn handle_approve_command(ws: &Workspace, bundle_id: String, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let bundle = client.approve_bundle(&bundle_id)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&bundle).context("serialize approve json")?
        );
    } else if bundle.promotable {
        println!("Approved {} (now promotable)", bundle.id);
    } else {
        println!(
            "Approved {} (still blocked: {:?})",
            bundle.id, bundle.reasons
        );
    }

    Ok(())
}
