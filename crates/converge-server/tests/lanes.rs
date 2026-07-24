//! g02.007 Batch 7.1: lane registry, ACLs, personal auto-provisioning,
//! and membership enforcement on publish — over real HTTP.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "whole-file".into(),
            }],
        },
    )?;
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-a".to_string(), "alice".to_string()),
            ("token-b".to_string(), "bob".to_string()),
        ]),
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt listener");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok(format!("http://{addr}"))
}

fn snap_in(dir: &std::path::Path, content: &str) -> Result<converge_client::model::SnapRecord> {
    let ws = Workspace::init(dir, false)?;
    std::fs::write(dir.join("f.txt"), content)?;
    ws.create_snap(None)
}

fn workspace(dir: &std::path::Path) -> Result<Workspace> {
    Workspace::discover(dir)
}

#[test]
fn lane_lifecycle_and_publish_enforcement() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Registry: create, duplicate rejected, list.
    let lane = alice.create_lane("repo", "feature/audio", "private")?;
    assert_eq!(lane.owner, "alice");
    assert!(
        alice
            .create_lane("repo", "feature/audio", "private")
            .is_err()
    );
    assert_eq!(alice.list_lanes("repo")?.len(), 1);

    // Membership managed by the owner only.
    assert!(bob.add_lane_member("repo", "feature/audio", "bob").is_err());
    alice.add_lane_member("repo", "feature/audio", "bob")?;

    // Publish to an unregistered lane is rejected.
    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "content")?;
    let ws = workspace(ws_dir.path())?;
    let err = alice
        .publish(
            &ws.store,
            "repo",
            "scope",
            "intake",
            &snap.id,
            &snap.root_manifest,
            None,
            Some("ghost-lane".into()),
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("not registered"));

    // Members may publish to the lane; provenance names it.
    let (bundle, _) = bob.publish(
        &ws.store,
        "repo",
        "scope",
        "intake",
        &snap.id,
        &snap.root_manifest,
        None,
        Some("feature/audio".into()),
        None,
    )?;
    assert!(bundle.inputs.len() == 1);

    Ok(())
}

#[test]
fn personal_lane_auto_provisions_when_no_lane_given() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "solo work")?;
    let ws = workspace(ws_dir.path())?;
    alice.publish(
        &ws.store,
        "repo",
        "scope",
        "intake",
        &snap.id,
        &snap.root_manifest,
        None,
        None,
        None,
    )?;

    let lanes = alice.list_lanes("repo")?;
    assert_eq!(lanes.len(), 1);
    assert_eq!(lanes[0].lane_id, "personal/alice");
    assert_eq!(lanes[0].owner, "alice");
    assert_eq!(lanes[0].visibility, "private");
    Ok(())
}

#[test]
fn non_member_cannot_publish_to_private_lane() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    alice.create_lane("repo", "alice-only", "private")?;

    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "intrusion")?;
    let ws = workspace(ws_dir.path())?;
    let err = bob
        .publish(
            &ws.store,
            "repo",
            "scope",
            "intake",
            &snap.id,
            &snap.root_manifest,
            None,
            Some("alice-only".into()),
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("not an owner or member"));
    Ok(())
}

#[test]
fn unpublished_sync_shares_lineage_between_clients() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Alice: two-snap lineage in a repo-visible shared lane.
    alice.create_lane("repo", "shared/wip", "repo")?;
    let ws_a_dir = tempfile::tempdir()?;
    let ws_a = Workspace::init(ws_a_dir.path(), false)?;
    std::fs::write(ws_a_dir.path().join("wip.txt"), "draft one")?;
    let s1 = ws_a.create_snap(Some("draft".into()))?;
    std::fs::write(ws_a_dir.path().join("wip.txt"), "draft two")?;
    let s2 = ws_a.create_snap(None)?;

    let head = alice.push_lineage(
        &ws_a.store,
        "repo",
        Some("shared/wip".into()),
        &s2.id,
        false,
    )?;
    assert_eq!(head.snap_id, s2.id);

    // Fast-forward rule: re-pushing the older snap is refused.
    let err = alice
        .push_lineage(
            &ws_a.store,
            "repo",
            Some("shared/wip".into()),
            &s1.id,
            false,
        )
        .unwrap_err();
    assert!(err.to_string().contains("non-fast-forward"));

    // Bob pulls, lineage intact, restores explicitly.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let pulled = bob.pull_lane(&ws_b.store, "repo", "shared/wip")?;
    assert_eq!(pulled, s2.id);
    let record = ws_b.store.get_snap(&pulled)?;
    assert_eq!(record.parents, vec![s1.id.clone()], "lineage intact");
    assert!(ws_b.store.has_snap(&s1.id), "ancestor record pulled");

    ws_b.restore_snap(&pulled, true)?;
    assert_eq!(
        std::fs::read_to_string(ws_b_dir.path().join("wip.txt"))?,
        "draft two"
    );
    Ok(())
}

#[test]
fn private_lane_head_not_readable_by_non_members() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("secret.txt"), "wip")?;
    let snap = ws.create_snap(None)?;
    // Personal lane (private) via default.
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;

    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let err = bob
        .pull_lane(&ws_b.store, "repo", "personal/alice")
        .unwrap_err();
    assert!(err.to_string().contains("private"));
    Ok(())
}
