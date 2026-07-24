//! Batches 11.3/11.4 (audit H3, M2): verify never mutates the object
//! store; batch endpoints and event listing are bounded.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn start_server(data_dir: &std::path::Path, seed_events: u64) -> Result<String> {
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
                strategy: "text-line-merge".into(),
                may_release: false,
            }],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    for i in 0..seed_events {
        meta.add_event(
            "repo",
            "bundle",
            &format!("seed-{i}"),
            "2026-07-24T00:00:00Z",
        )?;
    }

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([("token-a".to_string(), "alice".to_string())]),
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

/// Every object file under the store root: relative path -> size.
fn object_snapshot(data_dir: &std::path::Path) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let root = data_dir.join("objects");
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(meta) = path.metadata() {
                out.insert(
                    path.strip_prefix(&root).unwrap().display().to_string(),
                    meta.len(),
                );
            }
        }
    }
    out
}

#[test]
fn verify_leaves_the_object_store_byte_identical() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), 0)?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "line one\nline two\n")?;
    let snap = ws.create_snap(Some("s".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    let before = object_snapshot(server_dir.path());
    assert!(!before.is_empty(), "publish stored objects");
    let report = alice.verify(&bundle.bundle_id)?;
    assert!(report.verified, "replay reproduces the bundle");
    let after = object_snapshot(server_dir.path());
    assert_eq!(before, after, "verify (a GET) must not mutate the store");
    Ok(())
}

#[test]
fn over_cap_batch_requests_are_rejected() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), 0)?;
    let http = reqwest::blocking::Client::new();

    // batch-get: 4097 ids.
    let ids: Vec<String> = (0..4097).map(|i| format!("{i:064x}")).collect();
    let response = http
        .post(format!("{base_url}/api/repos/repo/objects/batch-get"))
        .bearer_auth("token-a")
        .json(&serde_json::json!({"blobs": ids, "manifests": [], "recipes": []}))
        .send()?;
    assert_eq!(response.status(), 400);
    assert!(response.text()?.contains("cap"));

    // batch upload: 4097 frames.
    #[derive(serde::Serialize)]
    struct Frame {
        kind: String,
        id: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    }
    let frames: Vec<Frame> = (0..4097)
        .map(|i| Frame {
            kind: "blobs".into(),
            id: format!("{i:064x}"),
            bytes: vec![0u8],
        })
        .collect();
    let mut body = Vec::new();
    ciborium::into_writer(&frames, &mut body)?;
    let response = http
        .post(format!("{base_url}/api/repos/repo/objects/batch"))
        .bearer_auth("token-a")
        .body(body)
        .send()?;
    assert_eq!(response.status(), 400);
    assert!(response.text()?.contains("cap"));
    Ok(())
}

#[test]
fn event_listing_is_paged_with_a_continuing_cursor() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), 1005)?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let first = alice.events("repo", 0)?;
    assert_eq!(first.len(), 1000, "one page per call");
    let cursor = first.last().expect("page").seq;
    let rest = alice.events("repo", cursor)?;
    assert_eq!(rest.len(), 5, "cursor continues past the page");
    Ok(())
}
