use super::*;

pub(in crate::cli_exec) fn handle_lanes_command(ws: &Workspace, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let mut lanes = client.list_lanes()?;
    lanes.sort_by(|a, b| a.id.cmp(&b.id));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&lanes).context("serialize lanes json")?
        );
    } else {
        for l in lanes {
            println!("lane: {}", l.id);
            let mut members = l.members.into_iter().collect::<Vec<_>>();
            members.sort();
            for m in members {
                if let Some(h) = l.heads.get(&m) {
                    let short = h.snap_id.chars().take(8).collect::<String>();
                    println!("  {} {} {}", m, short, h.updated_at);
                } else {
                    println!("  {} (no head)", m);
                }
            }
        }
    }

    Ok(())
}
