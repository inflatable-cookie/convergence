//! Batch 13.1 (audit H2): publish is one atomic guarded operation —
//! concurrent publishes to a single partition serialize through batch
//! guards instead of interleaving into inconsistent window state.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn start_server(data_dir: &std::path::Path, users: usize) -> Result<(String, Vec<String>)> {
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
                may_release: false,
            }],
        },
    )?;
    let mut tokens = HashMap::new();
    let mut token_list = Vec::new();
    for i in 0..users {
        let user = format!("user{i}");
        let token = format!("token{i}");
        meta.upsert_user(&user)?;
        for capability in ["read", "publish"] {
            meta.add_grant(&user, "repo", "*", capability)?;
        }
        tokens.insert(token.clone(), user);
        token_list.push(token);
    }

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens,
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
    Ok((format!("http://{addr}"), token_list))
}

#[test]
fn concurrent_publishes_serialize_into_consistent_windows() -> Result<()> {
    const PUBLISHERS: usize = 8;
    let server_dir = tempfile::tempdir()?;
    let (base_url, tokens) = start_server(server_dir.path(), PUBLISHERS)?;

    // Each publisher snaps distinct content, then all publish to the same
    // (repo, scope, gate) partition at once.
    let mut handles = Vec::new();
    for (i, token) in tokens.into_iter().enumerate() {
        let base_url = base_url.clone();
        handles.push(std::thread::spawn(move || -> Result<(u64, u64, usize)> {
            let ws_dir = tempfile::tempdir()?;
            let ws = Workspace::init(ws_dir.path(), false)?;
            std::fs::write(ws_dir.path().join(format!("f{i}.txt")), format!("body {i}"))?;
            let snap = ws.create_snap(None)?;
            let client = RemoteClient::new(&base_url, &token);
            let (bundle, _) = client.publish(
                &ws.store, "repo", "scope", "intake", &snap, None, None, None,
            )?;
            Ok((bundle.window.0, bundle.window.1, bundle.inputs.len()))
        }));
    }
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.join().expect("publisher thread")?);
    }

    // Serialized windows: every bundle starts at seq 1 (floor never moved),
    // the window ends are exactly 1..=N (no duplicate or lost seq), and the
    // bundle that closed the window at N folded every publication.
    let mut ends: Vec<u64> = results.iter().map(|(_, end, _)| *end).collect();
    ends.sort_unstable();
    assert_eq!(
        ends,
        (1..=PUBLISHERS as u64).collect::<Vec<_>>(),
        "each publish committed a distinct window end"
    );
    for (start, _, _) in &results {
        assert_eq!(*start, 1, "window always starts above the unmoved floor");
    }
    let full = results
        .iter()
        .find(|(_, end, _)| *end == PUBLISHERS as u64)
        .expect("some publish saw the full window");
    assert_eq!(
        full.2, PUBLISHERS,
        "the final window folds every publication"
    );
    Ok(())
}

// ---- Batch 13.2 (audit H1): promotion monotonicity guards ----

use converge_client::model::ManifestEntryKind;
use converge_server::{
    Capability, Engine, ObjectKind, ObjectStore, PartitionState, StoredBundle, authorize,
    storage::AssociatingObjects,
};

fn guard_setup(data: &std::path::Path) -> Result<SqliteMetadataStore> {
    let meta = SqliteMetadataStore::open(&data.join("meta.sqlite"))?;
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
                    may_release: false,
                },
                GateNode {
                    gate_id: "aux".into(),
                    name: "Aux".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
            ],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "promote"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    Ok(meta)
}

/// Snap a single distinct file locally, upload its tree, publish it.
fn publish_file(
    meta: &SqliteMetadataStore,
    objects: &FsObjectStore,
    name: &str,
) -> Result<StoredBundle> {
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join(name), format!("content of {name}"))?;
    let snap = ws.create_snap(None)?;

    let scoped = AssociatingObjects {
        inner: objects,
        meta,
        repo_id: "repo".into(),
    };
    let manifest = ws.store.get_manifest(&snap.root_manifest)?;
    for entry in &manifest.entries {
        if let ManifestEntryKind::File { blob, .. } = &entry.kind {
            scoped.put_bytes(ObjectKind::Blob, blob, &ws.store.get_blob(blob)?)?;
        }
    }
    scoped.put_bytes(
        ObjectKind::Manifest,
        &snap.root_manifest,
        &ws.store.get_manifest_bytes(&snap.root_manifest)?,
    )?;

    let engine = Engine {
        meta,
        objects: &scoped,
    };
    engine.publish(
        authorize(meta, "alice", "repo", "scope", Capability::Publish)?,
        converge_server::PublishInput {
            gate_id: "intake".into(),
            snap,
            base_bundle_id: None,
            lane_id: None,
            notes: None,
        },
    )
}

#[test]
fn stale_bundle_promote_refused_and_fanout_allowed() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = guard_setup(data.path())?;
    let objects = FsObjectStore::new(data.path());
    let engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let promote_authz =
        || authorize(&meta, "alice", "repo", "scope", Capability::Promote).expect("authz");

    let bundle_a = publish_file(&meta, &objects, "a.txt")?;
    let bundle_b = publish_file(&meta, &objects, "b.txt")?;
    assert_eq!(bundle_b.window, (1, 2));

    // Newest bundle promotes; the partition floor advances to 2.
    engine.promote(promote_authz(), &bundle_b.bundle_id, "main")?;
    assert_eq!(
        meta.get_partition_state("repo", "scope", "intake")?,
        PartitionState {
            window_floor: 2,
            base_bundle_id: Some(bundle_b.bundle_id.clone()),
        }
    );

    // Stale bundle (window ends at 1 <= floor 2) is refused loudly.
    let err = engine
        .promote(promote_authz(), &bundle_a.bundle_id, "main")
        .expect_err("stale promote must be refused");
    assert!(err.to_string().contains("stale bundle"), "got: {err}");
    assert_eq!(
        meta.get_partition_state("repo", "scope", "intake")?
            .window_floor,
        2,
        "floor never rewinds"
    );

    // Fan-out: the current W re-promotes to another downstream gate
    // without touching partition state.
    engine.promote(promote_authz(), &bundle_b.bundle_id, "aux")?;
    assert_eq!(
        meta.get_partition_state("repo", "scope", "intake")?,
        PartitionState {
            window_floor: 2,
            base_bundle_id: Some(bundle_b.bundle_id.clone()),
        }
    );
    assert_eq!(meta.list_promotions(&bundle_b.bundle_id)?.len(), 2);
    Ok(())
}

#[test]
fn wrong_base_promote_refused() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = guard_setup(data.path())?;
    let objects = FsObjectStore::new(data.path());
    let engine = Engine {
        meta: &meta,
        objects: &objects,
    };

    let bundle_b = publish_file(&meta, &objects, "b.txt")?;
    engine.promote(
        authorize(&meta, "alice", "repo", "scope", Capability::Promote)?,
        &bundle_b.bundle_id,
        "main",
    )?;
    // Built on W = bundle_b, window (2, 2).
    let bundle_c = publish_file(&meta, &objects, "c.txt")?;
    assert_eq!(bundle_c.base_bundle_id, Some(bundle_b.bundle_id.clone()));

    // The partition's W changes under bundle_c (simulated divergence).
    meta.set_partition_state(
        "repo",
        "scope",
        "intake",
        &PartitionState {
            window_floor: 1,
            base_bundle_id: Some("someone-elses-bundle".into()),
        },
    )?;
    let err = engine
        .promote(
            authorize(&meta, "alice", "repo", "scope", Capability::Promote)?,
            &bundle_c.bundle_id,
            "main",
        )
        .expect_err("wrong-base promote must be refused");
    assert!(
        err.to_string().contains("fork promoted history"),
        "got: {err}"
    );
    Ok(())
}
