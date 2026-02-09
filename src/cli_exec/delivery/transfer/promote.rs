use super::super::*;

pub(super) fn handle_promote_command(
    ws: &Workspace,
    bundle_id: String,
    to_gate: String,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let promotion = client.promote_bundle(&bundle_id, &to_gate)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&promotion).context("serialize promotion json")?
        );
    } else {
        println!("Promoted {} -> {}", promotion.from_gate, promotion.to_gate);
    }

    Ok(())
}
