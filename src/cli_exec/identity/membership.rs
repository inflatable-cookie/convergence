use super::*;

pub(crate) fn handle_members_command(ws: &Workspace, command: MembersCommands) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    match command {
        MembersCommands::List { json } => {
            let m = client.list_repo_members()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&m).context("serialize members json")?
                );
            } else {
                println!("owner: {}", m.owner);
                let publishers: std::collections::HashSet<String> =
                    m.publishers.into_iter().collect();
                let mut readers = m.readers;
                readers.sort();
                for r in readers {
                    let role = if publishers.contains(&r) {
                        "publish"
                    } else {
                        "read"
                    };
                    println!("{} {}", r, role);
                }
            }
        }
        MembersCommands::Add { handle, role, json } => {
            client.add_repo_member(&handle, &role)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "handle": handle, "role": role})
                );
            } else {
                println!("Added {} ({})", handle, role);
            }
        }
        MembersCommands::Remove { handle, json } => {
            client.remove_repo_member(&handle)?;
            if json {
                println!("{}", serde_json::json!({"ok": true, "handle": handle}));
            } else {
                println!("Removed {}", handle);
            }
        }
    }

    Ok(())
}

pub(crate) fn handle_lane_command(ws: &Workspace, command: LaneCommands) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    match command {
        LaneCommands::Members { lane_id, command } => match command {
            LaneMembersCommands::List { json } => {
                let m = client.list_lane_members(&lane_id)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&m).context("serialize lane members json")?
                    );
                } else {
                    println!("lane: {}", m.lane);
                    let mut members = m.members;
                    members.sort();
                    for h in members {
                        println!("{}", h);
                    }
                }
            }
            LaneMembersCommands::Add { handle, json } => {
                client.add_lane_member(&lane_id, &handle)?;
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"ok": true, "lane": lane_id, "handle": handle})
                    );
                } else {
                    println!("Added {} to lane {}", handle, lane_id);
                }
            }
            LaneMembersCommands::Remove { handle, json } => {
                client.remove_lane_member(&lane_id, &handle)?;
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"ok": true, "lane": lane_id, "handle": handle})
                    );
                } else {
                    println!("Removed {} from lane {}", handle, lane_id);
                }
            }
        },
    }

    Ok(())
}
