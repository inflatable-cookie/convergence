//! The gate graph write path (g02.026 batch 26.2).
//!
//! Batch 22.4 finding 33: the graph was write-once at repo creation, so
//! `promote` could not be reached by anyone. These tests are about the
//! guard rails on changing it, not about promotion itself — 26.4 drives
//! that on a real repo.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// A repo with the single `intake` gate `repo create` produces, which is
/// the state every existing deployment is in.
fn start_server(data_dir: &std::path::Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "scope", "2026-07-27T00:00:00Z")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "whole-file".into(),
                may_release: true,
            }],
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

fn gate(id: &str, upstreams: &[&str]) -> GateNode {
    GateNode {
        gate_id: id.into(),
        name: id.into(),
        upstreams: upstreams.iter().map(|s| s.to_string()).collect(),
        required_approvals: 0,
        strategy: "whole-file".into(),
        may_release: false,
    }
}

fn admin(server_dir: &std::path::Path) -> Result<()> {
    let meta = SqliteMetadataStore::open(&server_dir.join("meta.sqlite"))?;
    meta.add_grant("alice", "repo", "*", "admin")?;
    Ok(())
}

#[test]
fn a_repo_can_be_given_a_staged_graph_after_creation() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let mut release = gate("release", &["review"]);
    release.may_release = true;
    let staged = vec![gate("intake", &[]), gate("review", &["intake"]), release];

    let response = alice.set_gate_graph("repo", staged.clone(), None, false, false)?;
    assert!(response.applied);
    assert_eq!(response.impact.added.len(), 2, "{:?}", response.impact);

    let read_back = alice.get_gate_graph("repo")?;
    assert_eq!(read_back.gates.len(), 3);
    assert!(
        read_back
            .gates
            .iter()
            .any(|g| g.gate_id == "release" && g.may_release),
        "the release flag did not survive"
    );
    Ok(())
}

#[test]
fn an_illegal_graph_is_refused_with_every_reason() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    // A cycle would make `promote` walk upstreams forever.
    let err = alice
        .set_gate_graph(
            "repo",
            vec![gate("a", &["b"]), gate("b", &["a"])],
            None,
            false,
            false,
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cycle") || err.contains("nowhere to publish"),
        "{err}"
    );

    let mut bad_strategy = gate("intake", &[]);
    bad_strategy.strategy = "three-way-magic".into();
    let err = alice
        .set_gate_graph("repo", vec![bad_strategy], None, false, false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("three-way-magic"), "{err}");
    Ok(())
}

#[test]
fn only_an_admin_can_reshape_the_graph() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let bob = RemoteClient::new(&base_url, "token-b");

    let err = bob
        .set_gate_graph("repo", vec![gate("intake", &[])], None, false, false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("authorization denied"), "{err}");
    Ok(())
}

#[test]
fn a_change_that_would_strand_work_is_refused_then_forced() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let mut release = gate("release", &["intake"]);
    release.may_release = true;
    alice.set_gate_graph(
        "repo",
        vec![gate("intake", &[]), release.clone()],
        None,
        false,
        false,
    )?;

    // Put real work in `intake`.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("a.txt"), "work")?;
    let snap = ws.create_snap(Some("work".into()))?;
    alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    // Removing the gate that holds it is refused, and says what it holds.
    let only_release = vec![gate("release", &[])];
    let err = alice
        .set_gate_graph("repo", only_release.clone(), None, false, false)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("intake"),
        "the refusal did not name the gate: {err}"
    );
    assert!(
        err.contains("bundle"),
        "the refusal did not say what it holds: {err}"
    );

    // Nothing changed.
    assert_eq!(alice.get_gate_graph("repo")?.gates.len(), 2);

    // A dry run reports the same thing and still changes nothing.
    let dry = alice.set_gate_graph("repo", only_release.clone(), None, false, true)?;
    assert!(!dry.applied);
    assert!(dry.impact.strands_work());
    assert_eq!(alice.get_gate_graph("repo")?.gates.len(), 2);

    // Forcing works, because a repo whose graph can never be reshaped
    // once it holds a publication would have to be recreated instead.
    let forced = alice.set_gate_graph("repo", only_release, None, true, false)?;
    assert!(forced.applied);
    assert_eq!(alice.get_gate_graph("repo")?.gates.len(), 1);
    Ok(())
}

#[test]
fn a_concurrent_reshape_loses_rather_than_overwrites() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let before = alice.get_gate_graph("repo")?;

    // Somebody else reshapes first.
    alice.set_gate_graph(
        "repo",
        vec![gate("intake", &[]), gate("review", &["intake"])],
        None,
        false,
        false,
    )?;

    // Our edit, written against the graph we read, is refused.
    let err = alice
        .set_gate_graph(
            "repo",
            vec![gate("intake", &[]), gate("staging", &["intake"])],
            Some(before),
            false,
            false,
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("changed while you were editing"), "{err}");

    // And the first edit is intact: a lost update would have replaced it.
    let now = alice.get_gate_graph("repo")?;
    assert!(
        now.gates.iter().any(|g| g.gate_id == "review"),
        "the concurrent edit was overwritten"
    );

    // Re-reading and resubmitting works, which is the whole point.
    let current = alice.get_gate_graph("repo")?;
    let mut gates = current.gates.clone();
    gates.push(gate("staging", &["intake"]));
    assert!(
        alice
            .set_gate_graph("repo", gates, Some(current), false, false)?
            .applied
    );
    Ok(())
}

#[test]
fn a_graph_change_is_announced_on_the_event_feed() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    alice.set_gate_graph(
        "repo",
        vec![gate("intake", &[]), gate("review", &["intake"])],
        None,
        false,
        false,
    )?;

    let page = alice.events("repo", 0)?;
    assert!(
        page.iter().any(|e| e.kind == "gate.changed"),
        "another workspace has no way to learn the shape changed: {:?}",
        page.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );
    Ok(())
}

/// Scope before grant (batch 21.2), on a route added long after that
/// rule was written.
///
/// Batch 21.4 found twenty handlers that authorized their own way and so
/// ignored token scope entirely — one of them let a read-scoped token
/// grant itself admin. A new admin-only route is exactly where that
/// mistake recurs, so it is pinned here rather than assumed from the
/// fact that `authorize_repo` was called.
#[test]
fn an_admins_read_scoped_token_still_cannot_reshape_the_graph() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let narrow = alice.issue_token("repo", "reader", &["read".to_string()], None)?;
    let scoped = RemoteClient::new(&base_url, &narrow.token);

    // It can read the graph...
    assert!(scoped.get_gate_graph("repo").is_ok());

    // ...and not reshape it, even though its subject is an admin.
    let err = scoped
        .set_gate_graph("repo", vec![gate("intake", &[])], None, false, false)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("scoped to"),
        "the refusal should say it was the token, not the person: {err}"
    );
    Ok(())
}

/// The staged flow, which had never run before batch 26.4: a bundle
/// travelling intake -> review -> release and being released there.
///
/// Three defects made this impossible, all from one assumption — that a
/// bundle is only ever at the gate that produced it:
///
/// - promotion checked the target's upstreams against the *producing*
///   gate, so any gate whose upstream was not an entry gate was
///   unreachable
/// - `required_approvals` was read off the producing gate, so a review
///   stage's approval count was never enforced on the hop that leaves it
/// - `release` read `may_release` off the producing gate, which in a
///   staged graph is the entry gate
#[test]
fn a_bundle_travels_the_whole_staged_graph() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let mut review = gate("review", &["intake"]);
    review.required_approvals = 1;
    let mut release = gate("release", &["review"]);
    release.may_release = true;
    let mut intake = gate("intake", &[]);
    intake.may_release = false;
    alice.set_gate_graph("repo", vec![intake, review, release], None, false, false)?;

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("a.txt"), "staged")?;
    let snap = ws.create_snap(Some("staged".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    let id = bundle.bundle_id.clone();

    // Skipping a stage is still refused: release accepts only review.
    let err = alice.promote(&id, "repo", "scope", "release").unwrap_err();
    assert!(
        format!("{err:#}").contains("does not accept promotions"),
        "a stage was skippable: {err:#}"
    );

    // Leaving intake needs no approval; leaving review needs one.
    alice.promote(&id, "repo", "scope", "review")?;
    let err = alice.promote(&id, "repo", "scope", "release").unwrap_err();
    assert!(
        format!("{err:#}").contains("required approvals"),
        "the review gate's approval count was not enforced: {err:#}"
    );

    alice.approve(&id, "repo", "scope")?;
    alice.promote(&id, "repo", "scope", "release")?;

    // And it can be released from the gate it reached, not the one that
    // built it.
    alice.release(&id, "repo", "scope", "stable", None)?;
    assert_eq!(alice.get_channel_head("repo", "stable")?.bundle_id, id);
    Ok(())
}

/// An id given as a prefix must be stored resolved.
///
/// Batch 22.4 taught the server to accept shortened bundle ids, because
/// the CLI prints them. Batch 26.4 found what that cost: every verb that
/// *records* an id wrote back whatever the caller typed, so approvals,
/// promotions and releases all held twelve-character ids referencing no
/// real bundle. The promotion then failed to match the partition's base
/// and reported the bundle stale; worse, GC protects released bundles by
/// comparing ids, and a truncated id never matches.
#[test]
fn a_prefix_is_recorded_as_the_id_it_resolved_to() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    admin(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let mut release = gate("release", &["intake"]);
    release.may_release = true;
    let mut intake = gate("intake", &[]);
    intake.may_release = false;
    alice.set_gate_graph("repo", vec![intake, release], None, false, false)?;

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("a.txt"), "prefix")?;
    let snap = ws.create_snap(Some("prefix".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    let full = bundle.bundle_id.clone();
    let short = &full[..12];

    // Everything driven by the short form, the way a person would after
    // copying it out of `converge publish`.
    alice.approve(short, "repo", "scope")?;
    alice.promote(short, "repo", "scope", "release")?;
    alice.release(short, "repo", "scope", "stable", None)?;

    let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
    assert_eq!(
        meta.count_approvals(&full)?,
        1,
        "the approval was filed under the short id"
    );
    let promotions = meta.list_promotions(&full)?;
    assert_eq!(
        promotions.len(),
        1,
        "the promotion was filed under the short id"
    );
    let head = alice.get_channel_head("repo", "stable")?;
    assert_eq!(
        head.bundle_id, full,
        "the release recorded a truncated id, which GC would not match"
    );
    Ok(())
}
