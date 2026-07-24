use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{BundleStatus, GateGraph, GateNode, ManifestEntryKind, ObjectId};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// Spin the real HTTP server on an ephemeral port; returns its base URL.
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
                    required_approvals: 1,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
                GateNode {
                    gate_id: "main".into(),
                    name: "Main".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
            ],
        },
    )?;
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish", "resolve", "approve", "promote"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }
    for (lane, owner) in [("lane-a", "alice"), ("lane-b", "bob")] {
        meta.create_lane(&converge_model::LaneRecord {
            lane_id: lane.into(),
            repo_id: "repo".into(),
            owner: owner.into(),
            members: vec![],
            visibility: "repo".into(),
            created_at: "2026-07-24T00:00:00Z".into(),
        })?;
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
fn full_vertical_slice_over_http() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    // Two workspaces, divergent content for the same path.
    let ws_a_dir = tempfile::tempdir()?;
    let ws_a = Workspace::init(ws_a_dir.path(), false)?;
    std::fs::write(ws_a_dir.path().join("shared.txt"), "alice version")?;
    std::fs::write(ws_a_dir.path().join("common.txt"), "identical")?;
    let snap_a = ws_a.create_snap(Some("a".into()))?;

    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    std::fs::write(ws_b_dir.path().join("shared.txt"), "bob version")?;
    std::fs::write(ws_b_dir.path().join("common.txt"), "identical")?;
    let snap_b = ws_b.create_snap(Some("b".into()))?;

    let client_a = RemoteClient::new(&base_url, "token-a");
    let client_b = RemoteClient::new(&base_url, "token-b");

    // Publish A, then B — B's identical common.txt must dedup on negotiate.
    let (bundle_a, stats_a) = client_a.publish(
        &ws_a.store,
        "repo",
        "scope",
        "intake",
        &snap_a,
        None,
        Some("lane-a".into()),
        None,
    )?;
    assert!(stats_a.uploaded > 0);
    assert_eq!(bundle_a.status, BundleStatus::Ready { promotable: true });

    let (bundle_b, stats_b) = client_b.publish(
        &ws_b.store,
        "repo",
        "scope",
        "intake",
        &snap_b,
        None,
        Some("lane-b".into()),
        None,
    )?;
    // Dedup: only B's divergent blob + manifest travel; common blob is known.
    assert!(
        stats_b.uploaded < stats_a.uploaded,
        "expected dedup: {} vs {}",
        stats_b.uploaded,
        stats_a.uploaded
    );
    assert_eq!(bundle_b.status, BundleStatus::Ready { promotable: false });

    // Re-upload is idempotent and negotiates to zero (resume behavior).
    let stats_again = client_a.upload_tree(&ws_a.store, &snap_a.root_manifest)?;
    assert_eq!(stats_again.uploaded, 0, "everything already on server");

    // Fetch the superposed bundle into A's store and resolve locally.
    let root = client_a.fetch_bundle(&ws_a.store, &bundle_b.bundle_id)?;
    let manifest = ws_a.store.get_manifest(&root)?;
    let superposed = manifest
        .entries
        .iter()
        .find(|e| e.name == "shared.txt")
        .expect("shared.txt present");
    let variants = match &superposed.kind {
        ManifestEntryKind::Superposition { variants } => variants,
        other => panic!("expected superposition, got {other:?}"),
    };
    assert_eq!(variants.len(), 2);

    let decisions: std::collections::BTreeMap<String, converge_model::ResolutionDecision> =
        std::collections::BTreeMap::from([(
            "shared.txt".to_string(),
            converge_model::ResolutionDecision::Index(0),
        )]);
    let resolved_root = converge_client::resolve::apply_resolution(&ws_a.store, &root, &decisions)?;

    // Republish the resolved tree as a new snap from A.
    let resolved_snap = converge_model::SnapRecord {
        version: 2,
        id: converge_model::compute_snap_id(
            &resolved_root,
            std::slice::from_ref(&snap_a.id),
            Some(&bundle_b.bundle_id),
        ),
        created_at: "2026-07-24T00:00:00Z".into(),
        root_manifest: resolved_root,
        parents: vec![snap_a.id.clone()],
        derived_from_bundle: Some(bundle_b.bundle_id.clone()),
        message: None,
        trigger: "explicit".into(),
        stats: converge_model::SnapStats::default(),
    };
    let (resolved_bundle, _) = client_a.publish(
        &ws_a.store,
        "repo",
        "resolved-scope",
        "intake",
        &resolved_snap,
        None,
        Some("lane-a".into()),
        Some("resolution of shared.txt".into()),
    )?;
    assert_eq!(
        resolved_bundle.status,
        BundleStatus::Ready { promotable: true }
    );

    // Approve then promote through the gate graph.
    client_b.approve(&resolved_bundle.bundle_id, "repo", "resolved-scope")?;
    client_a.promote(&resolved_bundle.bundle_id, "repo", "resolved-scope", "main")?;

    // Promote without approvals is refused.
    let err = client_a
        .promote(&bundle_a.bundle_id, "repo", "scope", "main")
        .unwrap_err();
    assert!(err.to_string().contains("required approvals"));

    // Fetch resolved bundle into a fresh workspace and materialize.
    let ws_c_dir = tempfile::tempdir()?;
    let ws_c = Workspace::init(ws_c_dir.path(), false)?;
    let fetched_root: ObjectId = client_b.fetch_bundle(&ws_c.store, &resolved_bundle.bundle_id)?;
    let out = tempfile::tempdir()?;
    ws_c.materialize_manifest_to(&fetched_root, out.path(), true)?;
    assert_eq!(
        std::fs::read_to_string(out.path().join("shared.txt"))?,
        "alice version"
    );
    assert_eq!(
        std::fs::read_to_string(out.path().join("common.txt"))?,
        "identical"
    );
    Ok(())
}

#[test]
fn wrong_wire_version_refused() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let http = reqwest::blocking::Client::new();
    let response = http
        .post(format!("{base_url}/api/negotiate"))
        .bearer_auth("token-a")
        .json(&serde_json::json!({"wire_version": 999, "objects": {}}))
        .send()?;
    assert_eq!(response.status(), 400);
    assert!(response.text()?.contains("unsupported wire version"));
    Ok(())
}

#[test]
fn unknown_token_unauthorized() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let http = reqwest::blocking::Client::new();
    let response = http
        .post(format!("{base_url}/api/negotiate"))
        .bearer_auth("nope")
        .json(&serde_json::json!({"wire_version": 1, "objects": {}}))
        .send()?;
    assert_eq!(response.status(), 401);
    Ok(())
}
