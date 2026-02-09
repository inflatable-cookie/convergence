use super::super::*;

pub(super) fn handle_pins_command(ws: &Workspace, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let pins = client.list_pins()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&pins).context("serialize pins json")?
        );
    } else {
        for b in pins.bundles {
            println!("{}", b);
        }
    }

    Ok(())
}

pub(super) fn handle_pin_command(
    ws: &Workspace,
    bundle_id: String,
    unpin: bool,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    if unpin {
        client.unpin_bundle(&bundle_id)?;
    } else {
        client.pin_bundle(&bundle_id)?;
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "bundle_id": bundle_id,
                "pinned": !unpin
            })
        );
    } else if unpin {
        println!("Unpinned {}", bundle_id);
    } else {
        println!("Pinned {}", bundle_id);
    }

    Ok(())
}
