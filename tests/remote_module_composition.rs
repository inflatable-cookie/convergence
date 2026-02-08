mod common;

use anyhow::{Context, Result};
use converge::model::RemoteConfig;
use converge::remote::RemoteClient;
use converge::workspace::Workspace;

#[test]
fn remote_client_modules_compose_across_core_flows() -> Result<()> {
    let guard = common::spawn_server()?;
    let repo_id = "compose-smoke";
    let remote = RemoteConfig {
        base_url: guard.base_url.clone(),
        token: None,
        repo_id: repo_id.to_string(),
        scope: "main".to_string(),
        gate: "dev-intake".to_string(),
    };
    let client = RemoteClient::new(remote, guard.token.clone()).context("new remote client")?;

    // operations.rs
    let repo = client.create_repo(repo_id).context("create repo")?;
    assert_eq!(repo.id, repo_id);

    // identity.rs
    let who = client.whoami().context("whoami")?;
    assert!(!who.user.is_empty());

    // operations.rs (gate graph read)
    let graph = client.get_gate_graph().context("get gate graph")?;
    assert!(!graph.gates.is_empty());

    // transfer.rs: publish a snap.
    let ws_publish = tempfile::tempdir().context("create publish workspace")?;
    let publish_ws = Workspace::init(ws_publish.path(), false).context("init publish workspace")?;
    std::fs::write(ws_publish.path().join("a.txt"), b"hello\n").context("write a.txt")?;
    let snap = publish_ws
        .create_snap(Some("compose".to_string()))
        .context("create snap")?;

    let pubrec = client
        .publish_snap(&publish_ws.store, &snap, "main", "dev-intake")
        .context("publish snap")?;
    assert_eq!(pubrec.snap_id, snap.id);

    // fetch.rs: fetch the published snap into a separate store.
    let ws_fetch = tempfile::tempdir().context("create fetch workspace")?;
    let fetch_ws = Workspace::init(ws_fetch.path(), false).context("init fetch workspace")?;
    let fetched = client
        .fetch_publications(&fetch_ws.store, Some(&snap.id))
        .context("fetch publications")?;
    assert!(fetched.iter().any(|id| id == &snap.id));
    assert!(fetch_ws.store.has_snap(&snap.id));

    Ok(())
}
