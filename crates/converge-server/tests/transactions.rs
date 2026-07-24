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
