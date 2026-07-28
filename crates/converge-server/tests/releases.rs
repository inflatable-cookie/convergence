//! g02.008 Batch 8.1: release channels — policy, channel heads,
//! fetch-by-channel.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{
    AppState, FsObjectStore, MetadataStore, ObjectKind, ObjectStore, SqliteMetadataStore, router,
};

/// intake (no release) -> main (may_release).
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
                    may_release: true,
                },
            ],
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

#[test]
fn release_policy_channel_heads_and_fetch() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("app.txt"), "v1")?;
    let snap = ws.create_snap(None)?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    // Intake may not release, and the bundle has reached nowhere else.
    // The refusal names the whole path rather than one gate (batch
    // 26.4), because in a staged graph the interesting question is which
    // of the gates it has been through may release, not which one built
    // it.
    let err = alice
        .release(&bundle.bundle_id, "repo", "scope", "1.0.0", None)
        .unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("may release") && text.contains("intake"),
        "{text}"
    );

    // Promote to main, then release.
    alice.promote(&bundle.bundle_id, "repo", "scope", "main")?;
    // The promoted bundle now sits in intake's history; main's bundle is
    // produced by publishing into main... in this slice promotion records
    // movement, releases cut from the *producing* gate. Re-target: publish
    // directly to main (which may release).
    let (main_bundle, _) =
        alice.publish(&ws.store, "repo", "scope", "main", &snap, None, None, None)?;
    let release = alice.release(
        &main_bundle.bundle_id,
        "repo",
        "scope",
        // A leading v is accepted and stored bare (g02.028).
        "v1.0.0",
        Some("first".into()),
    )?;
    assert_eq!(release.version, "1.0.0");

    // A nonsense version is refused before anything is written.
    let err = alice
        .release(&main_bundle.bundle_id, "repo", "scope", "stable", None)
        .unwrap_err();
    assert!(err.to_string().contains("not a semver version"), "{err:#}");

    // And a duplicate is refused: releases are immutable, fix forward.
    let err = alice
        .release(&main_bundle.bundle_id, "repo", "scope", "1.0.0", None)
        .unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err:#}");

    // Capability enforced: bob (read-only) cannot release.
    let err = bob
        .release(&main_bundle.bundle_id, "repo", "scope", "1.1.0", None)
        .unwrap_err();
    assert!(err.to_string().contains("authorization denied"));

    // Channel head advances with a second release.
    std::fs::write(ws_dir.path().join("app.txt"), "v2")?;
    let snap2 = ws.create_snap(None)?;
    let (bundle2, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &snap2,
        Some(main_bundle.bundle_id.clone()),
        None,
        None,
    )?;
    alice.release(&bundle2.bundle_id, "repo", "scope", "1.2.0", None)?;
    let head = alice.resolve_release("repo", "latest")?;
    assert_eq!(head.bundle_id, bundle2.bundle_id, "channel head advanced");
    assert_eq!(alice.list_releases("repo")?.len(), 2);

    // Fetch by channel into a fresh workspace.
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root = bob.fetch_bundle(&ws_b.store, "repo", &head.bundle_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root, out.path(), true)?;
    assert_eq!(std::fs::read_to_string(out.path().join("app.txt"))?, "v2");
    Ok(())
}

#[test]
fn superposed_bundle_cannot_release() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_a = tempfile::tempdir()?;
    let a = Workspace::init(ws_a.path(), false)?;
    std::fs::write(ws_a.path().join("x.txt"), "one")?;
    let snap_a = a.create_snap(None)?;
    let ws_b = tempfile::tempdir()?;
    let b = Workspace::init(ws_b.path(), false)?;
    std::fs::write(ws_b.path().join("x.txt"), "two")?;
    let snap_b = b.create_snap(None)?;

    alice.publish(&a.store, "repo", "scope", "main", &snap_a, None, None, None)?;
    let (bundle, _) =
        alice.publish(&b.store, "repo", "scope", "main", &snap_b, None, None, None)?;
    let err = alice
        .release(&bundle.bundle_id, "repo", "scope", "1.0.0", None)
        .unwrap_err();
    assert!(err.to_string().contains("unresolved superpositions"));
    Ok(())
}

#[test]
fn retention_config_round_trips_with_admin_gate() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    // Grant alice admin for this test.
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    let policy = converge_model::RetentionPolicy {
        keep_releases: Some(5),
        keep_bundles_per_gate: Some(10),
        keep_publication_days: Some(30),
        keep_events: Some(1000),
    };
    alice.set_retention("repo", &policy)?;
    assert_eq!(alice.get_retention("repo")?, policy);
    assert_eq!(bob.get_retention("repo")?, policy, "readable by readers");

    let err = bob.set_retention("repo", &policy).unwrap_err();
    assert!(err.to_string().contains("authorization denied"));
    Ok(())
}

#[test]
fn gc_reclaims_unreachable_and_never_touches_reachable() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");

    // Reachable state: published + released bundle, lane lineage.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("keep.txt"), "released content")?;
    let snap = ws.create_snap(None)?;
    let (bundle, _) = alice.publish(&ws.store, "repo", "scope", "main", &snap, None, None, None)?;
    alice.release(&bundle.bundle_id, "repo", "scope", "1.0.0", None)?;
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;

    // Unreachable garbage: objects uploaded but never referenced.
    let objects = FsObjectStore::new(server_dir.path());
    let orphan_blob = objects.put(ObjectKind::Blob, b"orphaned bytes")?;
    let orphan_manifest = objects.put(ObjectKind::Manifest, br#"{"version":1,"entries":[]}"#)?;
    // Backdate mtimes past the grace window.
    for (kind, id) in [
        (ObjectKind::Blob, &orphan_blob),
        (ObjectKind::Manifest, &orphan_manifest),
    ] {
        let _ = kind;
        let _ = id;
    }
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    backdate_all(server_dir.path(), old)?;

    // Dry run reports but mutates nothing.
    let dry: serde_json::Value = alice.gc("repo", true)?;
    assert!(
        dry["swept_objects"].as_u64().unwrap() >= 2,
        "orphans counted"
    );
    assert!(
        objects.has(ObjectKind::Blob, &orphan_blob),
        "dry-run kept orphan"
    );

    // Execute: orphans gone, reachable state fully intact.
    let report: serde_json::Value = alice.gc("repo", false)?;
    assert!(report["swept_objects"].as_u64().unwrap() >= 2);
    assert!(!objects.has(ObjectKind::Blob, &orphan_blob), "orphan swept");
    assert!(!objects.has(ObjectKind::Manifest, &orphan_manifest));

    // Reachable-never-collected: channel head still fetches + materializes;
    // lane still pulls.
    let head = alice.resolve_release("repo", "latest")?;
    let ws_b_dir = tempfile::tempdir()?;
    let ws_b = Workspace::init(ws_b_dir.path(), false)?;
    let root = alice.fetch_bundle(&ws_b.store, "repo", &head.bundle_id)?;
    let out = tempfile::tempdir()?;
    ws_b.materialize_manifest_to(&root, out.path(), true)?;
    assert_eq!(
        std::fs::read_to_string(out.path().join("keep.txt"))?,
        "released content"
    );
    alice.pull_lane(&ws_b.store, "repo", "personal/alice")?;
    Ok(())
}

fn backdate_all(root: &std::path::Path, to: std::time::SystemTime) -> Result<()> {
    fn walk(dir: &std::path::Path, to: std::time::SystemTime) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, to)?;
            } else {
                let file = std::fs::File::options().append(true).open(&path)?;
                file.set_modified(to)?;
            }
        }
        Ok(())
    }
    let objects = root.join("objects");
    if objects.is_dir() {
        walk(&objects, to)?;
    }
    Ok(())
}

#[test]
fn gc_retention_drops_metadata_and_requires_admin() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");
    let bob = RemoteClient::new(&base_url, "token-b");

    // Two releases on one channel; retention keeps 1.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("a.txt"), "v1")?;
    let s1 = ws.create_snap(None)?;
    let (b1, _) = alice.publish(&ws.store, "repo", "scope", "main", &s1, None, None, None)?;
    alice.release(&b1.bundle_id, "repo", "scope", "1.0.0", None)?;
    std::fs::write(ws_dir.path().join("a.txt"), "v2")?;
    let s2 = ws.create_snap(None)?;
    let (b2, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &s2,
        Some(b1.bundle_id.clone()),
        None,
        None,
    )?;
    alice.release(&b2.bundle_id, "repo", "scope", "1.1.0", None)?;

    alice.set_retention(
        "repo",
        &converge_model::RetentionPolicy {
            keep_releases: Some(1),
            ..Default::default()
        },
    )?;

    let err = bob.gc("repo", true).unwrap_err();
    assert!(err.to_string().contains("authorization denied"));

    let report = alice.gc("repo", false)?;
    assert_eq!(report["dropped_releases"].as_u64().unwrap(), 1);
    assert_eq!(
        alice.list_releases("repo")?.len(),
        1,
        "old release row gone"
    );
    let head = alice.resolve_release("repo", "latest")?;
    assert_eq!(head.bundle_id, b2.bundle_id, "channel head survives");
    Ok(())
}

#[test]
fn verify_replays_provenance_and_detects_tamper() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    // Two-input bundle (supersession path) — richer replay.
    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("app.txt"), "v1")?;
    let s1 = ws.create_snap(None)?;
    let (b1, _) = alice.publish(&ws.store, "repo", "scope", "main", &s1, None, None, None)?;
    std::fs::write(ws_dir.path().join("app.txt"), "v2")?;
    let s2 = ws.create_snap(None)?;
    let (b2, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &s2,
        Some(b1.bundle_id.clone()),
        None,
        None,
    )?;

    let report = alice.verify(&b2.bundle_id)?;
    assert!(report.verified, "honest bundle verifies: {}", report.detail);
    assert_eq!(report.recomputed_root, b2.root_manifest);

    // Tamper: swap a recorded input root in the publication metadata.
    {
        use rusqlite::Connection;
        let conn = Connection::open(server_dir.path().join("meta.sqlite"))?;
        let tampered = format!(r#""root_manifest":"{}""#, s1.root_manifest.as_str());
        let original = format!(r#""root_manifest":"{}""#, s2.root_manifest.as_str());
        conn.execute(
            "UPDATE publications SET record_json = REPLACE(record_json, ?1, ?2)
             WHERE record_json LIKE '%' || ?3 || '%'",
            rusqlite::params![original, tampered, s2.id],
        )?;
    }
    let report = alice.verify(&b2.bundle_id)?;
    assert!(!report.verified, "tampered provenance must fail");
    Ok(())
}

#[test]
fn events_flow_with_increasing_seq_and_cursor_filtering() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;
    std::fs::write(ws_dir.path().join("e.txt"), "v1")?;
    let snap = ws.create_snap(None)?;

    // publish -> bundle event; sync push -> lane event; release -> release event.
    let (bundle, _) = alice.publish(&ws.store, "repo", "scope", "main", &snap, None, None, None)?;
    alice.push_lineage(&ws.store, "repo", None, &snap.id, false)?;
    alice.release(&bundle.bundle_id, "repo", "scope", "1.0.0", None)?;

    let events = alice.events("repo", 0)?;
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"bundle"));
    assert!(kinds.contains(&"lane"));
    assert!(kinds.contains(&"release"));
    assert!(
        events.windows(2).all(|w| w[0].seq < w[1].seq),
        "seq strictly increasing"
    );

    // Cursor filtering.
    let cursor = events[events.len() - 2].seq;
    let tail = alice.events("repo", cursor)?;
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].seq, events.last().unwrap().seq);
    assert!(alice.events("repo", tail[0].seq)?.is_empty());
    Ok(())
}

/// Retention must not drop a bundle a live publication was written
/// against, or publishing to that gate breaks permanently.
///
/// Batch 22.4 did this to a real repo with two ordinary commands:
/// `retention set --keep-bundles 5` then `gc --execute`. Publication 2
/// declared a base that GC deleted, so every later fold of the window
/// failed to load it. Two things made it terminal: publications only
/// leave a window when it advances, a window only advances on promotion,
/// and a single-gate repo cannot promote; and the client re-derives its
/// base and retries, so it never stops asking.
#[test]
fn retention_spares_bundles_that_open_publications_declare() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    {
        let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
        meta.add_grant("alice", "repo", "*", "admin")?;
    }
    let alice = RemoteClient::new(&base_url, "token-a");

    let ws_dir = tempfile::tempdir()?;
    let ws = Workspace::init(ws_dir.path(), false)?;

    // Enough publications that a tight keep-count wants to drop the
    // early ones, each declaring the previous bundle as its base.
    let mut base: Option<String> = None;
    for round in 0..6 {
        std::fs::write(ws_dir.path().join("a.txt"), format!("v{round}"))?;
        let snap = ws.create_snap(Some(format!("v{round}")))?;
        let (bundle, _) = alice.publish(
            &ws.store,
            "repo",
            "scope",
            "main",
            &snap,
            base.clone(),
            None,
            None,
        )?;
        assert!(
            !format!("{:?}", bundle.status).contains("ailed"),
            "publish {round} failed before retention was even set: {:?}",
            bundle.status
        );
        base = Some(bundle.bundle_id);
    }

    alice.set_retention(
        "repo",
        &converge_model::RetentionPolicy {
            keep_releases: None,
            keep_bundles_per_gate: Some(2),
            keep_publication_days: None,
            keep_events: None,
        },
    )?;
    let _: serde_json::Value = alice.gc("repo", false)?;

    // The next publish must still fold the window.
    std::fs::write(ws_dir.path().join("a.txt"), "after gc")?;
    let snap = ws.create_snap(Some("after gc".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store,
        "repo",
        "scope",
        "main",
        &snap,
        base.clone(),
        None,
        None,
    )?;
    assert!(
        !format!("{:?}", bundle.status).contains("ailed"),
        "retention wedged the gate: {:?}",
        bundle.status
    );
    Ok(())
}

/// A deployment that predates semver has channel-keyed rows *and* a
/// `channel` column with NOT NULL on it. Both halves matter: the rows
/// need real numbers (0.<n>.0 by order — operator's call, no legacy
/// caste), and the column has to go, because while it physically exists
/// every new insert fails — on the migrated deployment only. Every
/// fresh database, and therefore every test fixture, was fine, which is
/// why the whole suite passed minutes before the real deployment
/// refused to release (batch 28.2).
#[test]
fn a_pre_semver_deployment_migrates_and_can_still_release() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("meta.sqlite");
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE releases (
                repo_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                record_json TEXT NOT NULL
            );",
        )?;
        for (bundle, at) in [("b-one", "T1"), ("b-two", "T2")] {
            conn.execute(
                "INSERT INTO releases (repo_id, channel, record_json) VALUES ('repo', 'stable', ?1)",
                [format!(
                    r#"{{"channel":"stable","repo_id":"repo","scope_id":"s","bundle_id":"{bundle}","released_by":"tom","notes":null,"created_at":"{at}"}}"#
                )],
            )?;
        }
    }

    let meta = SqliteMetadataStore::open(&path)?;
    let migrated = meta.list_releases("repo")?;
    assert_eq!(migrated.len(), 2);
    assert_eq!(migrated[0].version, "0.1.0", "numbered by order");
    assert_eq!(migrated[1].version, "0.2.0");
    assert_eq!(migrated[0].bundle_id, "b-one", "history preserved");

    // The half the fixtures could not see: a *new* release on the
    // migrated schema.
    meta.add_release(&converge_model::ReleaseRecord {
        version: "1.0.0".into(),
        yanked: false,
        yank_reason: None,
        repo_id: "repo".into(),
        scope_id: "s".into(),
        bundle_id: "b-three".into(),
        released_by: "tom".into(),
        notes: None,
        created_at: "T3".into(),
    })?;
    assert_eq!(meta.list_releases("repo")?.len(), 3);
    assert!(meta.get_release("repo", "0.1.0")?.is_some());
    Ok(())
}
