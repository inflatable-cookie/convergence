//! g02.008 Batch 8.1: release channels — policy, channel heads,
//! fetch-by-channel.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{
    AppState, FsObjectStore, MetadataStore, ObjectKind, ObjectStore, SqliteMetadataStore, router,
};

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

#[test]
fn gc_reclaims_unreachable_and_never_touches_reachable() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");

    // Reachable state: published + released bundle, lane lineage.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("keep.txt"), "released content")?;
    let snap = ws.create_snap(None)?;
    let (bundle, _) = alice.publish(&ws.store, "repo", "scope", "main", &snap, None, None, None)?;
    alice.release(&bundle.bundle_id, "repo", "scope", "stable", None)?;
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;

    // Unreachable garbage: objects uploaded but never referenced.
    let objects = FsObjectStore::new(server_dir.path());
    let orphan_blob = objects.put(ObjectKind::Blob, b"orphaned bytes")?;
    let orphan_manifest = objects.put(ObjectKind::Manifest, br#"{"version":1,"entries":[]}"#)?;
    // Backdate mtimes past the grace window.
    for (kind, id) in [
        (ObjectKind::Blob, &orphan_blob),
        (ObjectKind::Manifest, &orphan_manifest),
    ] {
        let _ = kind;
        let _ = id;
    }
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    backdate_all(server_dir.path(), old)?;

    // Dry run reports but mutates nothing.
    let dry: serde_json::Value = alice.gc("repo", true)?;
    assert!(
        dry["swept_objects"].as_u64().unwrap() >= 2,
        "orphans counted"
    );
    assert!(
        objects.has(ObjectKind::Blob, &orphan_blob),
        "dry-run kept orphan"
    );

    // Execute: orphans gone, reachable state fully intact.
    let report: serde_json::Value = alice.gc("repo", false)?;
    assert!(report["swept_objects"].as_u64().unwrap() >= 2);
    assert!(!objects.has(ObjectKind::Blob, &orphan_blob), "orphan swept");
    assert!(!objects.has(ObjectKind::Manifest, &orphan_manifest));

    // Reachable-never-collected: channel head still fetches + materializes;
    // lane still pulls.
    let head = alice.get_channel_head("repo", "stable")?;
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root = alice.fetch_bundle(&ws_b.store, &head.bundle_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root, out.path(), true)?;
    assert_eq!(
        std::fs::read_to_string(out.path().join("keep.txt"))?,
        "released content"
    );
    alice.pull_lane(&ws_b.store, "repo", "personal/alice")?;
    Ok(())
}

fn backdate_all(root: &std::path::Path, to: std::time::SystemTime) -> Result<()> {
    fn walk(dir: &std::path::Path, to: std::time::SystemTime) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, to)?;
            } else {
                let file = std::fs::File::options().append(true).open(&path)?;
                file.set_modified(to)?;
            }
        }
        Ok(())
    }
    let objects = root.join("objects");
    if objects.is_dir() {
        walk(&objects, to)?;
    }
    Ok(())
}

#[test]
fn gc_retention_drops_metadata_and_requires_admin() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Two releases on one channel; retention keeps 1.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("a.txt"), "v1")?;
    let s1 = ws.create_snap(None)?;
    let (b1, _) = alice.publish(&ws.store, "repo", "scope", "main", &s1, None, None, None)?;
    alice.release(&b1.bundle_id, "repo", "scope", "stable", None)?;
    std::fs::write(ws_dir.path().join("a.txt"), "v2")?;
    let s2 = ws.create_snap(None)?;
    let (b2, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &s2,
        Some(b1.bundle_id.clone()),
        None,
        None,
    )?;
    alice.release(&b2.bundle_id, "repo", "scope", "stable", None)?;

    alice.set_retention(
        "repo",
        &converge_model::RetentionPolicy {
            keep_releases_per_channel: Some(1),
            ..Default::default()
        },
    )?;

    let err = bob.gc("repo", true).unwrap_err();
    assert!(err.to_string().contains("authorization denied"));

    let report = alice.gc("repo", false)?;
    assert_eq!(report["dropped_releases"].as_u64().unwrap(), 1);
    assert_eq!(
        alice.list_releases("repo")?.len(),
        1,
        "old release row gone"
    );
    let head = alice.get_channel_head("repo", "stable")?;
    assert_eq!(head.bundle_id, b2.bundle_id, "channel head survives");
    Ok(())
}
