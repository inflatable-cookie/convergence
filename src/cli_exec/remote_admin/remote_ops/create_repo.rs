use super::*;

pub(super) fn create_repo(ws: &Workspace, repo: Option<String>, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let repo_id = repo.unwrap_or_else(|| remote.repo_id.clone());
    let client = RemoteClient::new(remote, token)?;
    let created = client.create_repo(&repo_id)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&created).context("serialize repo create json")?
        );
    } else {
        println!("Created repo {}", created.id);
    }
    Ok(())
}
