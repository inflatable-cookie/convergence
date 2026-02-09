use super::*;

pub(super) fn show_remote(ws: &Workspace, json: bool) -> Result<()> {
    let cfg = ws.store.read_config()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&cfg.remote).context("serialize remote json")?
        );
    } else if let Some(remote) = cfg.remote {
        println!("url: {}", remote.base_url);
        println!("repo: {}", remote.repo_id);
        println!("scope: {}", remote.scope);
        println!("gate: {}", remote.gate);
    } else {
        println!("No remote configured");
    }
    Ok(())
}

pub(super) fn set_remote(
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
    println!("Remote configured");
    Ok(())
}
