//! Batch 11.2 (audit C3, L2, arch-1.4): personal-lane reservation and the
//! snap-sync capability boundary.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// One repo; alice holds publish, carol holds read+snap-sync only, dave
/// holds nothing.
fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    // Scopes are declared repo state (batch 14.3).
    for scope in ["scope", "resolved-scope"] {
        meta.create_scope("repo", scope, "2026-07-25T00:00:00Z")?;
    }
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "whole-file".into(),
                may_release: false,
            }],
        },
    )?;
    for (subject, capabilities) in [
        ("alice", &["read", "publish"][..]),
        ("carol", &["read", "snap-sync"][..]),
        ("dave", &[][..]),
    ] {
        meta.upsert_user(subject)?;
        for capability in capabilities {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-alice".to_string(), "alice".to_string()),
            ("token-carol".to_string(), "carol".to_string()),
            ("token-dave".to_string(), "dave".to_string()),
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
fn personal_lane_namespace_is_reserved() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-alice");

    // Another subject's personal lane: refused.
    let err = alice
        .create_lane("repo", "personal/carol", "repo")
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("reserved"),
        "expected reservation refusal, got: {err:#}"
    );

    // One's own personal lane: allowed (e.g. to widen visibility).
    alice.create_lane("repo", "personal/alice", "repo")?;

    // Publishing with no lane still lands in the (pre-created) personal
    // lane owned by the publisher, not anyone else.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "content")?;
    let snap = ws.create_snap(Some("s".into()))?;
    alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    let lanes = alice.list_lanes("repo")?;
    let personal = lanes
        .iter()
        .find(|l| l.lane_id == "personal/alice")
        .expect("personal lane");
    assert_eq!(personal.owner, "alice");
    Ok(())
}

#[test]
fn snap_sync_capability_boundary() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-alice");
    let carol = RemoteClient::new(&base_url, "token-carol");
    let dave = RemoteClient::new(&base_url, "token-dave");

    // Carol (snap-sync, no publish) can push unpublished lineage…
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("wip.txt"), "carol wip")?;
    let snap = ws.create_snap(Some("wip".into()))?;
    let head = carol.push_lineage(&ws.store, "repo", None, &snap.id, false)?;
    assert_eq!(head.lane_id, "personal/carol");

    // …but cannot publish into a gate.
    let err = carol
        .publish(
            &ws.store, "repo", "scope", "intake", &snap, None, None, None,
        )
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("403"),
        "expected publish denial, got: {err:#}"
    );

    // Alice's publish grant subsumes snap-sync (arch 14 §4).
    let ws2_dir = tempfile::tempdir()?;
    let ws2 = Workspace::init(ws2_dir.path(), false)?;
    std::fs::write(ws2_dir.path().join("a.txt"), "alice wip")?;
    let snap2 = ws2.create_snap(Some("a".into()))?;
    alice.push_lineage(&ws2.store, "repo", None, &snap2.id, false)?;

    // Dave (no grants) can neither sync nor manage lane membership.
    assert!(
        dave.push_lineage(&ws.store, "repo", None, &snap.id, false)
            .is_err()
    );
    let err = dave
        .add_lane_member("repo", "personal/carol", "dave")
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("403"),
        "expected membership denial, got: {err:#}"
    );
    Ok(())
}
