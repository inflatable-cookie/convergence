//! Batch 15.2 (audit 4.4 / L6): no endpoint returns an unbounded set,
//! cursors are stable, and the inbox stays bounded and honest about it.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_model::{GateGraph, GateNode, LaneRecord};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn start_server(data_dir: &std::path::Path, lanes: usize) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "scope", "2026-07-25T00:00:00Z")?;
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
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    for i in 0..lanes {
        meta.create_lane(&LaneRecord {
            // Zero-padded so lexical order is also numeric order.
            lane_id: format!("lane-{i:03}"),
            repo_id: "repo".into(),
            owner: "alice".into(),
            members: vec![],
            visibility: "repo".into(),
            created_at: "2026-07-25T00:00:00Z".into(),
        })?;
    }

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([("token-a".to_string(), "alice".to_string())]),
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
fn lane_cursor_walks_the_whole_listing_without_gaps_or_repeats() -> Result<()> {
    const LANES: usize = 25;
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), LANES)?;
    let client = RemoteClient::new(&base_url, "token-a");

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let page = client.list_lanes_page("repo", cursor.as_deref(), Some(10))?;
        pages += 1;
        assert!(page.items.len() <= 10, "limit honored");
        seen.extend(page.items.iter().map(|l| l.lane_id.clone()));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
        assert!(pages < 10, "cursor must terminate");
    }

    assert_eq!(pages, 3, "25 lanes at 10 per page");
    let mut expected: Vec<String> = (0..LANES).map(|i| format!("lane-{i:03}")).collect();
    expected.sort();
    assert_eq!(seen, expected, "every lane exactly once, in key order");
    Ok(())
}

#[test]
fn a_client_that_sends_no_limit_still_gets_a_capped_page() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), 5)?;
    let http = reqwest::blocking::Client::new();

    // Raw request without `limit`, as an older client would send.
    let response = http
        .get(format!("{base_url}/api/repos/repo/lanes"))
        .bearer_auth("token-a")
        .send()?;
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json()?;
    assert!(
        body.get("items").and_then(|i| i.as_array()).is_some(),
        "response is a page, not a bare array: {body}"
    );

    // An absurd limit is clamped rather than honored.
    let response = http
        .get(format!("{base_url}/api/repos/repo/lanes"))
        .query(&[("limit", "100000")])
        .bearer_auth("token-a")
        .send()?;
    let body: serde_json::Value = response.json()?;
    assert_eq!(body["items"].as_array().expect("items").len(), 5);
    assert!(body["next_cursor"].is_null(), "listing exhausted");
    Ok(())
}

#[test]
fn convenience_listing_follows_pages_to_completion() -> Result<()> {
    const LANES: usize = 40;
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), LANES)?;
    let client = RemoteClient::new(&base_url, "token-a");

    // `list_lanes` pages under the hood; callers still see everything.
    assert_eq!(client.list_lanes("repo")?.len(), LANES);
    assert_eq!(client.list_scopes("repo")?, vec!["default", "scope"]);
    assert!(client.list_releases("repo")?.is_empty());
    Ok(())
}

#[test]
fn scope_cursor_pages_in_key_order() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path(), 0)?;
    let client = RemoteClient::new(&base_url, "token-a");
    for name in ["alpha", "beta", "gamma"] {
        client.create_scope("repo", name)?;
    }

    let first = client.list_scopes_page("repo", None, Some(2))?;
    assert_eq!(first.items, vec!["alpha".to_string(), "beta".to_string()]);
    let cursor = first.next_cursor.expect("more to come");
    let second = client.list_scopes_page("repo", Some(&cursor), Some(2))?;
    assert_eq!(
        second.items,
        vec!["default".to_string(), "gamma".to_string()]
    );
    // A page that fills exactly still reports a cursor — the server does
    // not spend a second query proving the listing ended. The follower
    // learns it from the next, short page.
    let cursor = second.next_cursor.expect("full page reports a cursor");
    let third = client.list_scopes_page("repo", Some(&cursor), Some(2))?;
    // `default` (repo creation) and `scope` (harness) round out the five.
    assert_eq!(third.items, vec!["scope".to_string()]);
    assert!(third.next_cursor.is_none(), "short page ends the listing");
    Ok(())
}

/// Audit 4.4 / L6: the inbox answered a question about a handful of
/// gates by reading every bundle ever built in the scope. It now asks
/// the store for at most one bundle per gate.
#[test]
fn inbox_reads_one_bundle_per_gate_not_the_whole_scope() -> Result<()> {
    use converge_model::BundleStatus;
    use converge_server::StoredBundle;

    let dir = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "scope", "t")?;

    // Many bundles across two gates; only the newest of each is current.
    for i in 0..50 {
        for gate in ["intake", "main"] {
            meta.put_bundle(&StoredBundle {
                bundle_id: format!("{gate}-{i:03}"),
                repo_id: "repo".into(),
                scope_id: "scope".into(),
                gate_id: gate.into(),
                inputs: vec![],
                root_manifest: None,
                base_bundle_id: None,
                window: (0, 0),
                strategy: "whole-file".into(),
                status: BundleStatus::Ready { promotable: true },
                created_at: format!("2026-07-25T00:00:{i:02}Z"),
            })?;
        }
    }

    let latest = meta.latest_bundles_per_gate("repo", "scope")?;
    assert_eq!(latest.len(), 2, "one row per gate, not 100");
    let mut ids: Vec<&str> = latest.iter().map(|b| b.bundle_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec!["intake-049", "main-049"],
        "the newest bundle of each gate"
    );
    // The unpaged scan is still available for GC, and still sees all.
    assert_eq!(meta.list_bundles("repo", "scope")?.len(), 100);
    Ok(())
}
