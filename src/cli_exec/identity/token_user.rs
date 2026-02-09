use super::*;

pub(crate) fn handle_token_command(ws: &Workspace, command: TokenCommands) -> Result<()> {
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

pub(crate) fn handle_user_command(ws: &Workspace, command: UserCommands) -> Result<()> {
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
