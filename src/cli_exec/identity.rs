use super::*;

pub(super) fn handle_token_command(ws: &Workspace, command: TokenCommands) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    match command {
        TokenCommands::Create { label, user, json } => {
            let created = if let Some(handle) = user.as_deref() {
                let users = client.list_users()?;
                let uid = users
                    .iter()
                    .find(|u| u.handle == handle)
                    .map(|u| u.id.clone())
                    .with_context(|| format!("unknown user handle: {}", handle))?;
                client.create_token_for_user(&uid, label)?
            } else {
                client.create_token(label)?
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&created)
                        .context("serialize token create json")?
                );
            } else {
                println!("token_id: {}", created.id);
                println!("token: {}", created.token);
                println!("created_at: {}", created.created_at);
                println!("note: token is shown once; store it now");
            }
        }
        TokenCommands::List { json } => {
            let list = client.list_tokens()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).context("serialize token list json")?
                );
            } else {
                for t in list {
                    let label = t.label.unwrap_or_default();
                    let revoked = if t.revoked_at.is_some() {
                        " revoked"
                    } else {
                        ""
                    };
                    println!("{} {}{}", t.id, label, revoked);
                }
            }
        }
        TokenCommands::Revoke { id, json } => {
            client.revoke_token(&id)?;
            if json {
                println!("{}", serde_json::json!({"revoked": true, "id": id}));
            } else {
                println!("Revoked {}", id);
            }
        }
    }

    Ok(())
}

pub(super) fn handle_user_command(ws: &Workspace, command: UserCommands) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    match command {
        UserCommands::List { json } => {
            let mut users = client.list_users()?;
            users.sort_by(|a, b| a.handle.cmp(&b.handle));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&users).context("serialize users json")?
                );
            } else {
                for u in users {
                    let admin = if u.admin { " admin" } else { "" };
                    println!("{} {}{}", u.id, u.handle, admin);
                }
            }
        }
        UserCommands::Create {
            handle,
            display_name,
            admin,
            json,
        } => {
            let created = client.create_user(&handle, display_name, admin)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&created).context("serialize user create json")?
                );
            } else {
                println!("user_id: {}", created.id);
                println!("handle: {}", created.handle);
                println!("admin: {}", created.admin);
            }
        }
    }

    Ok(())
}

pub(super) fn handle_members_command(ws: &Workspace, command: MembersCommands) -> Result<()> {
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

pub(super) fn handle_lane_command(ws: &Workspace, command: LaneCommands) -> Result<()> {
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

pub(super) fn handle_login_command(
    ws: &Workspace,
    url: String,
    token: String,
    repo: String,
    scope: String,
    gate: String,
) -> Result<()> {
    let mut cfg = ws.store.read_config()?;
    let remote = converge::model::RemoteConfig {
        base_url: url,
        token: None,
        repo_id: repo,
        scope,
        gate,
    };
    ws.store
        .set_remote_token(&remote, &token)
        .context("store remote token in state.json")?;
    cfg.remote = Some(remote);
    ws.store.write_config(&cfg)?;
    println!("Logged in");
    Ok(())
}

pub(super) fn handle_logout_command(ws: &Workspace) -> Result<()> {
    let cfg = ws.store.read_config()?;
    let remote = cfg
        .remote
        .context("no remote configured (run `converge login --url ... --token ... --repo ...`)")?;
    ws.store
        .clear_remote_token(&remote)
        .context("clear remote token")?;
    println!("Logged out");
    Ok(())
}

pub(super) fn handle_whoami_command(ws: &Workspace, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let who = client.whoami()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&who).context("serialize whoami json")?
        );
    } else {
        println!("user: {}", who.user);
        println!("user_id: {}", who.user_id);
        println!("admin: {}", who.admin);
    }
    Ok(())
}
