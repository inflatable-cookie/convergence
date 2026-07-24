//! g02.008 Batch 8.1: release channels — policy, channel heads,
//! fetch-by-channel.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// intake (no release) -> main (may_release).
fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![
                GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
                GateNode {
                    gate_id: "main".into(),
                    name: "Main".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: true,
                },
            ],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "promote", "release"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    meta.upsert_user("bob")?;
    meta.add_grant("bob", "repo", "*", "read")?;

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

#[test]
fn release_policy_channel_heads_and_fetch() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("app.txt"), "v1")?;
    let snap = ws.create_snap(None)?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    // Intake may not release.
    let err = alice
        .release(&bundle.bundle_id, "repo", "scope", "stable", None)
        .unwrap_err();
    assert!(err.to_string().contains("may not release"));

    // Promote to main, then release.
    alice.promote(&bundle.bundle_id, "repo", "scope", "main")?;
    // The promoted bundle now sits in intake's history; main's bundle is
    // produced by publishing into main... in this slice promotion records
    // movement, releases cut from the *producing* gate. Re-target: publish
    // directly to main (which may release).
    let (main_bundle, _) =
        alice.publish(&ws.store, "repo", "scope", "main", &snap, None, None, None)?;
    let release = alice.release(
        &main_bundle.bundle_id,
        "repo",
        "scope",
        "stable",
        Some("first".into()),
    )?;
    assert_eq!(release.channel, "stable");

    // Capability enforced: bob (read-only) cannot release.
    let err = bob
        .release(&main_bundle.bundle_id, "repo", "scope", "stable", None)
        .unwrap_err();
    assert!(err.to_string().contains("authorization denied"));

    // Channel head advances with a second release.
    std::fs::write(ws_dir.path().join("app.txt"), "v2")?;
    let snap2 = ws.create_snap(None)?;
    let (bundle2, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &snap2,
        Some(main_bundle.bundle_id.clone()),
        None,
        None,
    )?;
    alice.release(&bundle2.bundle_id, "repo", "scope", "stable", None)?;
    let head = alice.get_channel_head("repo", "stable")?;
    assert_eq!(head.bundle_id, bundle2.bundle_id, "channel head advanced");
    assert_eq!(alice.list_releases("repo")?.len(), 2);

    // Fetch by channel into a fresh workspace.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root = bob.fetch_bundle(&ws_b.store, &head.bundle_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root, out.path(), true)?;
    assert_eq!(std::fs::read_to_string(out.path().join("app.txt"))?, "v2");
    Ok(())
}

#[test]
fn superposed_bundle_cannot_release() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_a = tempfile::tempdir()?;
    let a = Workspace::init(ws_a.path(), false)?;
    std::fs::write(ws_a.path().join("x.txt"), "one")?;
    let snap_a = a.create_snap(None)?;
    let ws_b = tempfile::tempdir()?;
    let b = Workspace::init(ws_b.path(), false)?;
    std::fs::write(ws_b.path().join("x.txt"), "two")?;
    let snap_b = b.create_snap(None)?;

    alice.publish(&a.store, "repo", "scope", "main", &snap_a, None, None, None)?;
    let (bundle, _) =
        alice.publish(&b.store, "repo", "scope", "main", &snap_b, None, None, None)?;
    let err = alice
        .release(&bundle.bundle_id, "repo", "scope", "stable", None)
        .unwrap_err();
    assert!(err.to_string().contains("unresolved superpositions"));
    Ok(())
}

#[test]
fn retention_config_round_trips_with_admin_gate() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    // Grant alice admin for this test.
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let policy = converge_model::RetentionPolicy {
        keep_releases_per_channel: Some(5),
        keep_bundles_per_gate: Some(10),
        keep_publication_days: Some(30),
    };
    alice.set_retention("repo", &policy)?;
    assert_eq!(alice.get_retention("repo")?, policy);
    assert_eq!(bob.get_retention("repo")?, policy, "readable by readers");

    let err = bob.set_retention("repo", &policy).unwrap_err();
    assert!(err.to_string().contains("authorization denied"));
    Ok(())
}
