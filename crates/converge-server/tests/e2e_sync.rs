use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{CandidateStatus, GateGraph, GateNode, ManifestEntryKind, ObjectId};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// Spin the real HTTP server on an ephemeral port; returns its base URL.
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
        gc_running: Default::default(),
        oidc: None,
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
    let (candidate_a, stats_a) = client_a.publish(
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
    assert_eq!(
        candidate_a.status,
        CandidateStatus::Ready { promotable: true }
    );

    let (candidate_b, stats_b) = client_b.publish(
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
    assert_eq!(
        candidate_b.status,
        CandidateStatus::Ready { promotable: false }
    );

    // Re-upload is idempotent and negotiates to zero (resume behavior).
    let stats_again = client_a.upload_tree(&ws_a.store, "repo", &snap_a.root_manifest)?;
    assert_eq!(stats_again.uploaded, 0, "everything already on server");

    // Fetch the superposed candidate into A's store and resolve locally.
    let root = client_a.fetch_candidate(&ws_a.store, "repo", &candidate_b.candidate_id)?;
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
            Some(&candidate_b.candidate_id),
        ),
        created_at: "2026-07-24T00:00:00Z".into(),
        root_manifest: resolved_root,
        parents: vec![snap_a.id.clone()],
        derived_from_candidate: Some(candidate_b.candidate_id.clone()),
        message: None,
        trigger: "explicit".into(),
        stats: converge_model::SnapStats::default(),
    };
    let (resolved_candidate, _) = client_a.publish(
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
        resolved_candidate.status,
        CandidateStatus::Ready { promotable: true }
    );

    // Approve then promote through the gate graph.
    client_b.approve(&resolved_candidate.candidate_id, "repo", "resolved-scope")?;
    client_a.promote(
        &resolved_candidate.candidate_id,
        "repo",
        "resolved-scope",
        "main",
    )?;

    // Promote without approvals is refused.
    let err = client_a
        .promote(&candidate_a.candidate_id, "repo", "scope", "main")
        .unwrap_err();
    assert!(err.to_string().contains("required approvals"));

    // Fetch resolved candidate into a fresh workspace and materialize.
    let ws_c_dir = tempfile::tempdir()?;
    let ws_c = Workspace::init(ws_c_dir.path(), false)?;
    let fetched_root: ObjectId =
        client_b.fetch_candidate(&ws_c.store, "repo", &resolved_candidate.candidate_id)?;
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
        .post(format!("{base_url}/api/repos/repo/negotiate"))
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
        .post(format!("{base_url}/api/repos/repo/negotiate"))
        .bearer_auth("nope")
        .json(&serde_json::json!({"wire_version": 1, "objects": {}}))
        .send()?;
    assert_eq!(response.status(), 401);
    Ok(())
}

#[test]
fn batch_cap_splitting_round_trips_a_larger_tree() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    // Tiny cap: every frame flushes its own batch.
    let client = RemoteClient::new(&base_url, "token-a").with_batch_cap(64);

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    for i in 0..20 {
        std::fs::write(
            ws_dir.path().join(format!("f{i}.txt")),
            format!("content {i}"),
        )?;
    }
    std::fs::create_dir(ws_dir.path().join("sub"))?;
    std::fs::write(ws_dir.path().join("sub/inner.txt"), "nested")?;
    let snap = ws.create_snap(None)?;

    let (candidate, stats) = client.publish(
        &ws.store,
        "repo",
        "scope",
        "intake",
        &snap,
        None,
        Some("lane-a".into()),
        None,
    )?;
    assert!(stats.uploaded >= 22, "all objects travelled");
    assert_eq!(
        candidate.status,
        CandidateStatus::Ready { promotable: true }
    );

    // Fetch into a fresh store via batch-get waves and materialize.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root = client.fetch_candidate(&ws_b.store, "repo", &candidate.candidate_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root, out.path(), true)?;
    assert_eq!(
        std::fs::read_to_string(out.path().join("f7.txt"))?,
        "content 7"
    );
    assert_eq!(
        std::fs::read_to_string(out.path().join("sub/inner.txt"))?,
        "nested"
    );
    Ok(())
}

/// Server FS path for an object (mirrors FsObjectStore sharding).
fn object_path(data_dir: &std::path::Path, kind_dir: &str, id: &str) -> std::path::PathBuf {
    data_dir
        .join("objects")
        .join(kind_dir)
        .join(&id[..2])
        .join(&id[2..4])
        .join(id)
}

/// Audit C4: a torn upload can leave leaf holes under manifests the
/// server already has. Re-upload must detect and heal them instead of
/// pruning on "server has manifest ⇒ has subtree".
#[test]
fn torn_server_tree_heals_on_reupload() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let client = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("top.txt"), "top content")?;
    std::fs::create_dir(ws_dir.path().join("sub"))?;
    std::fs::write(ws_dir.path().join("sub/inner.txt"), "inner content")?;
    let snap = ws.create_snap(None)?;

    let (candidate, _) = client.publish(
        &ws.store,
        "repo",
        "scope",
        "intake",
        &snap,
        None,
        Some("lane-a".into()),
        None,
    )?;

    // Simulate the tear: a leaf blob under the still-present root
    // manifest, plus a child manifest, vanish server-side.
    let root = ws.store.get_manifest(&snap.root_manifest)?;
    let top_blob = root
        .entries
        .iter()
        .find_map(|e| match &e.kind {
            ManifestEntryKind::File { blob, .. } if e.name == "top.txt" => Some(blob.clone()),
            _ => None,
        })
        .expect("top.txt blob");
    let sub_manifest = root
        .entries
        .iter()
        .find_map(|e| match &e.kind {
            ManifestEntryKind::Dir { manifest } => Some(manifest.clone()),
            _ => None,
        })
        .expect("sub manifest");
    for (kind, id) in [("blobs", &top_blob), ("manifests", &sub_manifest)] {
        let path = object_path(server_dir.path(), kind, id.as_str());
        assert!(path.exists(), "{kind} object present before tear");
        std::fs::remove_file(path)?;
    }

    // Re-upload heals both holes.
    let stats = client.upload_tree(&ws.store, "repo", &snap.root_manifest)?;
    assert!(
        stats.uploaded >= 2,
        "holes re-uploaded, got {}",
        stats.uploaded
    );

    // Full tree fetches and materializes from a fresh store.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root_id = client.fetch_candidate(&ws_b.store, "repo", &candidate.candidate_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root_id, out.path(), true)?;
    assert_eq!(
        std::fs::read_to_string(out.path().join("top.txt"))?,
        "top content"
    );
    assert_eq!(
        std::fs::read_to_string(out.path().join("sub/inner.txt"))?,
        "inner content"
    );
    Ok(())
}

/// A thinned ancestor (absent server-side, 404) is a legitimate gap:
/// the pull succeeds and stops the walk there.
#[test]
fn pull_lane_tolerates_thinned_ancestor() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let client = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "v1")?;
    let snap1 = ws.create_snap(None)?;
    std::fs::write(ws_dir.path().join("f.txt"), "v2")?;
    let snap2 = ws.create_snap(None)?;
    assert_eq!(snap2.parents, vec![snap1.id.clone()]);

    // Upload only the head: its tree, its record, the lane head — the
    // parent record never reaches the server (thinned).
    client.upload_tree(&ws.store, "repo", &snap2.root_manifest)?;
    let http = reqwest::blocking::Client::new();
    let response = http
        .put(format!("{base_url}/api/repos/repo/snaps/{}", snap2.id))
        .bearer_auth("token-a")
        .json(&snap2)
        .send()?;
    assert!(response.status().is_success(), "{}", response.text()?);
    let response = http
        .post(format!("{base_url}/api/repos/repo/lane-head"))
        .bearer_auth("token-a")
        .json(&serde_json::json!({
            "lane_id": "lane-a",
            "snap_id": snap2.id,
            "force": false,
        }))
        .send()?;
    assert!(response.status().is_success(), "{}", response.text()?);

    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let pulled = client.pull_lane(&ws_b.store, "repo", "lane-a")?;
    assert_eq!(pulled, snap2.id);
    assert!(ws_b.store.has_snap(&snap2.id));
    assert!(
        !ws_b.store.has_snap(&snap1.id),
        "thinned parent stays absent"
    );
    Ok(())
}

/// Audit C5: a non-404 failure mid-walk must fail the pull, not
/// masquerade as a thinned gap.
#[test]
fn pull_lane_fails_loudly_on_server_error() -> Result<()> {
    // Stub server: healthy lane head, 500 on every snap record.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let app = axum::Router::new()
                .route(
                    "/api/repos/:repo/lane-head/:lane",
                    axum::routing::get(|| async {
                        axum::Json(serde_json::json!({
                            "lane_id": "lane-a",
                            "snap_id": "deadbeef",
                            "updated_at": "2026-07-24T00:00:00Z",
                        }))
                    }),
                )
                .route(
                    "/api/repos/:repo/snaps/:id",
                    axum::routing::get(|| async {
                        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom")
                    }),
                );
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt listener");
            axum::serve(listener, app).await.expect("serve");
        });
    });

    let client = RemoteClient::new(&format!("http://{addr}"), "token-a");
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    let err = client
        .pull_lane(&ws.store, "repo", "lane-a")
        .expect_err("500 mid-walk must fail the pull");
    assert!(
        err.to_string().contains("500"),
        "error names the status: {err}"
    );
    Ok(())
}
