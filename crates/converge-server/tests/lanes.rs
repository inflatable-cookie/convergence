//! g02.007 Batch 7.1: lane registry, ACLs, personal auto-provisioning,
//! and membership enforcement on publish — over real HTTP.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

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
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-a".to_string(), "alice".to_string()),
            ("token-b".to_string(), "bob".to_string()),
        ]),
        gc_running: Default::default(),
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

fn snap_in(dir: &std::path::Path, content: &str) -> Result<converge_client::model::SnapRecord> {
    let ws = Workspace::init(dir, false)?;
    std::fs::write(dir.join("f.txt"), content)?;
    ws.create_snap(None)
}

fn workspace(dir: &std::path::Path) -> Result<Workspace> {
    Workspace::discover(dir)
}

#[test]
fn lane_lifecycle_and_publish_enforcement() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Registry: create, duplicate rejected, list.
    let lane = alice.create_lane("repo", "feature/audio", "private")?;
    assert_eq!(lane.owner, "alice");
    assert!(
        alice
            .create_lane("repo", "feature/audio", "private")
            .is_err()
    );
    assert_eq!(alice.list_lanes("repo")?.len(), 1);

    // Membership managed by the owner only.
    assert!(bob.add_lane_member("repo", "feature/audio", "bob").is_err());
    alice.add_lane_member("repo", "feature/audio", "bob")?;

    // Publish to an unregistered lane is rejected.
    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "content")?;
    let ws = workspace(ws_dir.path())?;
    let err = alice
        .publish(
            &ws.store,
            "repo",
            "scope",
            "intake",
            &snap,
            None,
            Some("ghost-lane".into()),
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("not registered"));

    // Members may publish to the lane; provenance names it.
    let (bundle, _) = bob.publish(
        &ws.store,
        "repo",
        "scope",
        "intake",
        &snap,
        None,
        Some("feature/audio".into()),
        None,
    )?;
    assert!(bundle.inputs.len() == 1);

    Ok(())
}

#[test]
fn personal_lane_auto_provisions_when_no_lane_given() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "solo work")?;
    let ws = workspace(ws_dir.path())?;
    alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    let lanes = alice.list_lanes("repo")?;
    assert_eq!(lanes.len(), 1);
    assert_eq!(lanes[0].lane_id, "personal/alice");
    assert_eq!(lanes[0].owner, "alice");
    assert_eq!(lanes[0].visibility, "private");
    Ok(())
}

#[test]
fn non_member_cannot_publish_to_private_lane() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    alice.create_lane("repo", "alice-only", "private")?;

    let ws_dir = tempfile::tempdir()?;
    let snap = snap_in(ws_dir.path(), "intrusion")?;
    let ws = workspace(ws_dir.path())?;
    let err = bob
        .publish(
            &ws.store,
            "repo",
            "scope",
            "intake",
            &snap,
            None,
            Some("alice-only".into()),
            None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("not an owner or member"));
    Ok(())
}

#[test]
fn unpublished_sync_shares_lineage_between_clients() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Alice: two-snap lineage in a repo-visible shared lane.
    alice.create_lane("repo", "shared/wip", "repo")?;
    let ws_a_dir = tempfile::tempdir()?;
    let ws_a = Workspace::init(ws_a_dir.path(), false)?;
    std::fs::write(ws_a_dir.path().join("wip.txt"), "draft one")?;
    let s1 = ws_a.create_snap(Some("draft".into()))?;
    std::fs::write(ws_a_dir.path().join("wip.txt"), "draft two")?;
    let s2 = ws_a.create_snap(None)?;

    let head = alice.push_lineage(
        &ws_a.store,
        "repo",
        Some("shared/wip".into()),
        &s2.id,
        false,
    )?;
    assert_eq!(head.snap_id, s2.id);

    // Fast-forward rule: re-pushing the older snap is refused.
    let err = alice
        .push_lineage(
            &ws_a.store,
            "repo",
            Some("shared/wip".into()),
            &s1.id,
            false,
        )
        .unwrap_err();
    assert!(err.to_string().contains("non-fast-forward"));

    // Bob pulls, lineage intact, restores explicitly.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let pulled = bob.pull_lane(&ws_b.store, "repo", "shared/wip")?;
    assert_eq!(pulled, s2.id);
    let record = ws_b.store.get_snap(&pulled)?;
    assert_eq!(record.parents, vec![s1.id.clone()], "lineage intact");
    assert!(ws_b.store.has_snap(&s1.id), "ancestor record pulled");

    ws_b.restore_snap(&pulled, true)?;
    assert_eq!(
        std::fs::read_to_string(ws_b_dir.path().join("wip.txt"))?,
        "draft two"
    );
    Ok(())
}

#[test]
fn private_lane_head_not_readable_by_non_members() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("secret.txt"), "wip")?;
    let snap = ws.create_snap(None)?;
    // Personal lane (private) via default.
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;

    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let err = bob
        .pull_lane(&ws_b.store, "repo", "personal/alice")
        .unwrap_err();
    assert!(err.to_string().contains("private"));
    Ok(())
}

#[test]
fn inbox_reports_visible_activity_and_recommendations() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Private WIP from alice + shared lane activity.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "wip")?;
    let snap = ws.create_snap(None)?;
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;
    alice.create_lane("repo", "shared/wip", "repo")?;
    alice.push_lineage(
        &ws.store,
        "repo",
        Some("shared/wip".into()),
        &snap.id,
        false,
    )?;

    // Divergent publishes -> superposed bundle needing resolution.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    std::fs::write(ws_b_dir.path().join("f.txt"), "other")?;
    let snap_b = ws_b.create_snap(None)?;
    alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    bob.publish(
        &ws_b.store,
        "repo",
        "scope",
        "intake",
        &snap_b,
        None,
        None,
        None,
    )?;

    // Bob's inbox: sees the shared lane but not alice's personal lane;
    // sees the superposed bundle with a resolve recommendation.
    let report = bob.inbox("repo", "scope", None)?;
    let lane_ids: Vec<&str> = report.lanes.iter().map(|l| l.lane_id.as_str()).collect();
    assert!(lane_ids.contains(&"shared/wip"));
    assert!(
        !lane_ids.iter().any(|l| l.starts_with("personal/alice")),
        "private lane hidden"
    );
    assert_eq!(
        report.publications.len(),
        2,
        "open window publications listed"
    );
    assert_eq!(report.bundles.len(), 1);
    assert_eq!(report.bundles[0].recommendation, "resolve");

    // Alice's inbox additionally shows her personal lane.
    let report = alice.inbox("repo", "scope", None)?;
    assert!(report.lanes.iter().any(|l| l.lane_id == "personal/alice"));
    Ok(())
}

#[test]
fn inbox_recommends_approval_when_short() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    // Raise required approvals on intake for this server.
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.set_gate_graph(
            "repo",
            &GateGraph {
                gates: vec![GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 2,
                    strategy: "whole-file".into(),
                    may_release: false,
                }],
            },
        )?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("f.txt"), "clean")?;
    let snap = ws.create_snap(None)?;
    alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    let report = alice.inbox("repo", "scope", None)?;
    assert_eq!(report.bundles.len(), 1);
    assert_eq!(report.bundles[0].recommendation, "approve");
    assert_eq!(report.bundles[0].required_approvals, 2);
    Ok(())
}

#[test]
fn provenance_chain_is_complete_and_variant_sources_are_lanes() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Divergent publishes over personal lanes.
    let ws_a_dir = tempfile::tempdir()?;
    let snap_a = snap_in(ws_a_dir.path(), "alice version")?;
    let ws_a = workspace(ws_a_dir.path())?;
    let (bundle_a, _) = alice.publish(
        &ws_a.store,
        "repo",
        "scope",
        "intake",
        &snap_a,
        None,
        None,
        None,
    )?;

    let ws_b_dir = tempfile::tempdir()?;
    let snap_b = snap_in(ws_b_dir.path(), "bob version")?;
    let ws_b = workspace(ws_b_dir.path())?;
    let (bundle, _) = bob.publish(
        &ws_b.store,
        "repo",
        "scope",
        "intake",
        &snap_b,
        Some(bundle_a.bundle_id.clone()),
        None,
        None,
    )?;

    // Provenance answers who / where-from / on-what for every input.
    let provenance = alice.get_provenance(&bundle.bundle_id)?;
    assert_eq!(provenance.inputs.len(), 2);
    let by_publisher: std::collections::HashMap<&str, &converge_model::PublicationRecord> =
        provenance
            .inputs
            .iter()
            .map(|i| (i.publisher.as_str(), i))
            .collect();
    assert_eq!(by_publisher["alice"].lane_id, "personal/alice");
    assert_eq!(
        by_publisher["bob"].base_bundle_id.as_deref(),
        Some(bundle_a.bundle_id.as_str())
    );
    assert_eq!(by_publisher["alice"].snap_id, snap_a.id);

    // Snap records stored on publish (no separate sync needed).
    let fetched: converge_model::SnapRecord = reqwest::blocking::Client::new()
        .get(format!("{base_url}/api/repos/repo/snaps/{}", snap_a.id))
        .bearer_auth("token-b")
        .send()?
        .json()?;
    assert_eq!(fetched.root_manifest, snap_a.root_manifest);

    // Variant sources are registered lanes.
    let registered: std::collections::HashSet<String> = alice
        .list_lanes("repo")?
        .into_iter()
        .map(|l| l.lane_id)
        .collect();
    let root = bundle.root_manifest.expect("root");
    let ws_check_dir = tempfile::tempdir()?;
    let ws_check = Workspace::init(ws_check_dir.path(), false)?;
    alice
        .pull_lane(&ws_check.store, "repo", "personal/alice")
        .ok();
    // Fetch the bundle tree and inspect variants.
    alice.fetch_bundle(&ws_check.store, "repo", &bundle.bundle_id)?;
    let manifest = ws_check.store.get_manifest(&root)?;
    for entry in &manifest.entries {
        if let converge_client::model::ManifestEntryKind::Superposition { variants } = &entry.kind {
            for variant in variants {
                assert!(
                    registered.contains(&variant.source),
                    "variant source {} is not a registered lane",
                    variant.source
                );
            }
        }
    }
    Ok(())
}

#[test]
fn tampered_snap_record_rejected_on_publish() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let mut snap = snap_in(ws_dir.path(), "honest")?;
    let ws = workspace(ws_dir.path())?;
    snap.parents = vec!["forged-parent".into()]; // id no longer matches
    let err = alice
        .publish(
            &ws.store, "repo", "scope", "intake", &snap, None, None, None,
        )
        .unwrap_err();
    assert!(err.to_string().contains("identity mismatch"));
    Ok(())
}
