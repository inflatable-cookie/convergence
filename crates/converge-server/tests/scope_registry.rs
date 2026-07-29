//! Batch 14.3 (audit 2.4 / M3): scopes are declared repo state, so a
//! typo cannot mint a partition and fragment windows; grant scope
//! patterns mean what their name claims.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{
    AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router,
    storage::scope_pattern_matches,
};

fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "frontend", "2026-07-25T00:00:00Z")?;
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

fn snap_in(dir: &std::path::Path) -> Result<(Workspace, converge_model::SnapRecord)> {
    let ws = Workspace::init(dir, false)?;
    std::fs::write(dir.join("a.txt"), "content")?;
    let snap = ws.create_snap(None)?;
    Ok((ws, snap))
}

#[test]
fn publish_to_unregistered_scope_is_refused_and_writes_nothing() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let client = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let (ws, snap) = snap_in(ws_dir.path())?;

    // "frontnd" is a typo for the registered "frontend".
    let err = client
        .publish(
            &ws.store, "repo", "frontnd", "intake", &snap, None, None, None,
        )
        .expect_err("unregistered scope must be refused");
    let message = err.to_string();
    assert!(message.contains("unknown scope frontnd"), "got: {message}");
    assert!(
        message.contains("frontend"),
        "error lists the registered scopes: {message}"
    );

    // Nothing was minted: the repo still knows only what was declared.
    assert_eq!(
        client.list_scopes("repo")?,
        vec!["default".to_string(), "frontend".to_string()]
    );

    // The registered scope publishes fine.
    let (candidate, _) = client.publish(
        &ws.store, "repo", "frontend", "intake", &snap, None, None, None,
    )?;
    assert_eq!(candidate.scope_id, "frontend");
    Ok(())
}

#[test]
fn scopes_can_be_registered_then_used() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let client = RemoteClient::new(&base_url, "token-a");

    client.create_scope("repo", "backend")?;
    let mut scopes = client.list_scopes("repo")?;
    scopes.sort();
    assert_eq!(
        scopes,
        vec![
            "backend".to_string(),
            "default".to_string(),
            "frontend".to_string()
        ],
        "repo creation registers `default`"
    );

    let ws_dir = tempfile::tempdir()?;
    let (ws, snap) = snap_in(ws_dir.path())?;
    let (candidate, _) = client.publish(
        &ws.store, "repo", "backend", "intake", &snap, None, None, None,
    )?;
    assert_eq!(candidate.scope_id, "backend");
    Ok(())
}

/// Grant patterns: `*`, a literal scope, or `prefix/*` — and nothing
/// else is a wildcard.
#[test]
fn scope_patterns_match_exactly_what_they_claim() {
    assert!(scope_pattern_matches("*", "anything"));
    assert!(scope_pattern_matches("frontend", "frontend"));
    assert!(!scope_pattern_matches("frontend", "backend"));

    assert!(scope_pattern_matches("team/*", "team/web"));
    assert!(scope_pattern_matches("team/*", "team/web/deep"));
    assert!(!scope_pattern_matches("team/*", "team"));
    assert!(
        !scope_pattern_matches("team/*", "teams/web"),
        "prefix must end at a path boundary"
    );
    assert!(
        !scope_pattern_matches("team/*", "other/team/web"),
        "prefix anchors at the start"
    );

    // Bare `*` suffixes are not wildcards; they are literal.
    assert!(!scope_pattern_matches("front*", "frontend"));
    assert!(scope_pattern_matches("front*", "front*"));
}

#[test]
fn prefix_grant_authorizes_only_its_subtree() -> Result<()> {
    let data = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&data.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    for scope in ["team/web", "team/api", "other/thing"] {
        meta.create_scope("repo", scope, "2026-07-25T00:00:00Z")?;
    }
    meta.upsert_user("bob")?;
    meta.add_grant("bob", "repo", "team/*", "publish")?;

    assert!(meta.has_grant("bob", "repo", "team/web", "publish")?);
    assert!(meta.has_grant("bob", "repo", "team/api", "publish")?);
    assert!(!meta.has_grant("bob", "repo", "other/thing", "publish")?);
    assert!(
        !meta.has_grant("bob", "repo", "team/web", "promote")?,
        "the pattern does not widen capabilities"
    );
    Ok(())
}
