use super::*;

pub(crate) fn handle_login_command(
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

pub(crate) fn handle_logout_command(ws: &Workspace) -> Result<()> {
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

pub(crate) fn handle_whoami_command(ws: &Workspace, json: bool) -> Result<()> {
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
