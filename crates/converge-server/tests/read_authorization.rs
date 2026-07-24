//! Batch 11.1 (audit C1): read endpoints must prove repo membership.
//! Two repos on one shared object store; a subject granted on repo-b only
//! must not read repo-a content by hash or bundle id.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode, ObjectSet};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn gate_graph() -> GateGraph {
    GateGraph {
        gates: vec![GateNode {
            gate_id: "intake".into(),
            name: "Intake".into(),
            upstreams: vec![],
            required_approvals: 0,
            strategy: "whole-file".into(),
            may_release: false,
        }],
    }
}

/// Server with `repo-a` (alice's grants) and `repo-b` (bob's grants);
/// neither subject holds anything on the other's repo.
fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    for repo in ["repo-a", "repo-b"] {
        meta.create_repo(repo)?;
        meta.set_gate_graph(repo, &gate_graph())?;
    }
    for (subject, repo) in [("alice", "repo-a"), ("bob", "repo-b")] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish", "approve", "promote"] {
            meta.add_grant(subject, repo, "*", capability)?;
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

#[test]
fn cross_repo_reads_are_denied() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Alice publishes secret content into repo-a.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("secret.txt"), "repo-a secret")?;
    let snap = ws.create_snap(Some("secret".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo-a", "scope", "intake", &snap, None, None, None,
    )?;
    let root = bundle.root_manifest.clone().expect("merged root");

    // Bob (repo-b only) cannot read repo-a's bundle, provenance, or verify.
    for err in [
        bob.get_bundle(&bundle.bundle_id).unwrap_err(),
        bob.get_provenance(&bundle.bundle_id).unwrap_err(),
        bob.verify(&bundle.bundle_id).unwrap_err(),
    ] {
        let msg = format!("{err:#}");
        assert!(msg.contains("404"), "expected 404, got: {msg}");
        assert!(
            !msg.contains("secret") && !msg.contains("scope"),
            "leaked detail: {msg}"
        );
    }

    // Bob cannot pull repo-a's objects by hash — via repo-a routes (no
    // grant) or via his own repo-b routes (no association).
    let fetch_dir = tempfile::tempdir()?;
    let fetch_ws = Workspace::init(fetch_dir.path(), false)?;
    assert!(
        bob.fetch_bundle(&fetch_ws.store, "repo-a", &bundle.bundle_id)
            .is_err()
    );
    assert!(
        bob.fetch_bundle(&fetch_ws.store, "repo-b", &bundle.bundle_id)
            .is_err()
    );

    // Negotiate on repo-b treats repo-a's manifest as missing (existence
    // is not disclosed across repos)…
    let missing = bob.negotiate(
        "repo-b",
        ObjectSet {
            manifests: vec![root.clone()],
            ..Default::default()
        },
    )?;
    assert_eq!(missing.manifests, vec![root.clone()]);
    // …and negotiate on repo-a is refused outright.
    assert!(
        bob.negotiate(
            "repo-a",
            ObjectSet {
                manifests: vec![root.clone()],
                ..Default::default()
            },
        )
        .is_err()
    );

    // Alice still reads her own bundle and tree.
    assert_eq!(
        alice.get_bundle(&bundle.bundle_id)?.root_manifest,
        Some(root)
    );
    let ok_dir = tempfile::tempdir()?;
    let ok_ws = Workspace::init(ok_dir.path(), false)?;
    alice.fetch_bundle(&ok_ws.store, "repo-a", &bundle.bundle_id)?;
    Ok(())
}

#[test]
fn shared_object_readable_from_both_repos_when_uploaded_to_both() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Identical content in both repos: dedup keeps one stored object with
    // two associations, each side reads its own.
    let mk = |name: &str| -> Result<(tempfile::TempDir, Workspace, converge_model::SnapRecord)> {
        let dir = tempfile::tempdir()?;
        let ws = Workspace::init(dir.path(), false)?;
        std::fs::write(dir.path().join("shared.txt"), "same bytes")?;
        let snap = ws.create_snap(Some(name.into()))?;
        Ok((dir, ws, snap))
    };
    let (_dir_ws_a, ws_a, snap_a) = mk("a")?;
    let (_dir_ws_b, ws_b, snap_b) = mk("b")?;

    let (bundle_a, _) = alice.publish(
        &ws_a.store,
        "repo-a",
        "scope",
        "intake",
        &snap_a,
        None,
        None,
        None,
    )?;
    let (bundle_b, _) = bob.publish(
        &ws_b.store,
        "repo-b",
        "scope",
        "intake",
        &snap_b,
        None,
        None,
        None,
    )?;

    let dir_a = tempfile::tempdir()?;
    let fetch_a = Workspace::init(dir_a.path(), false)?;
    alice.fetch_bundle(&fetch_a.store, "repo-a", &bundle_a.bundle_id)?;

    let dir_b = tempfile::tempdir()?;
    let fetch_b = Workspace::init(dir_b.path(), false)?;
    bob.fetch_bundle(&fetch_b.store, "repo-b", &bundle_b.bundle_id)?;
    Ok(())
}
