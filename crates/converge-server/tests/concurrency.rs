//! Batch 18.1: a standing multi-writer harness.
//!
//! The audit found zero concurrency tests, which is how every race it
//! reported managed to ship. Batch 13.1-13.2 fixed the races and added
//! single-threaded guard tests; this file drives the same guards with
//! real clients on real threads, so the next race is caught here rather
//! than by the next audit.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use converge_client::model::BundleStatus;
use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// A running server plus the tokens to hit it with. One `(repo, scope)`
/// partition, two gates, so promotion has somewhere to go.
struct Cluster {
    base_url: String,
    tokens: Vec<String>,
    _dir: tempfile::TempDir,
}

impl Cluster {
    fn start(clients: usize) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let meta = SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?;
        meta.create_repo("repo")?;
        meta.create_scope("repo", "scope", "2026-07-25T00:00:00Z")?;
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

        let mut tokens = HashMap::new();
        let mut token_list = Vec::new();
        for i in 0..clients {
            let subject = format!("user{i}");
            let token = format!("token-{i}");
            meta.upsert_user(&subject)?;
            for capability in ["read", "publish", "resolve", "approve", "promote", "admin"] {
                meta.add_grant(&subject, "repo", "*", capability)?;
            }
            tokens.insert(token.clone(), subject);
            token_list.push(token);
        }

        let state = AppState {
            meta: Arc::new(meta),
            objects: Arc::new(FsObjectStore::new(dir.path())),
            tokens,
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
        Ok(Self {
            base_url: format!("http://{addr}"),
            tokens: token_list,
            _dir: dir,
        })
    }

    fn client(&self, i: usize) -> RemoteClient {
        RemoteClient::new(&self.base_url, &self.tokens[i % self.tokens.len()])
    }
}

/// A throwaway workspace holding one file, snapped.
fn snap_with(
    name: &str,
    body: &str,
) -> Result<(tempfile::TempDir, Workspace, converge_model::SnapRecord)> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    std::fs::write(dir.path().join(name), body)?;
    let snap = ws.create_snap(Some(body.into()))?;
    Ok((dir, ws, snap))
}

/// Publishes and promotions racing on one partition. Promotion advances
/// the window floor, so this is the interleaving that can lose or double
/// count a publication if the guards are wrong (audit H1).
#[test]
fn promotions_racing_publishes_keep_windows_contiguous() -> Result<()> {
    const PUBLISHERS: usize = 6;
    const PROMOTERS: usize = 2;
    let cluster = Cluster::start(PUBLISHERS + PROMOTERS)?;

    let mut publishers = Vec::new();
    for i in 0..PUBLISHERS {
        let client = cluster.client(i);
        publishers.push(std::thread::spawn(move || -> Result<(u64, u64)> {
            let (_dir, ws, snap) = snap_with(&format!("f{i}.txt"), &format!("body {i}"))?;
            let (bundle, _) = client.publish(
                &ws.store, "repo", "scope", "intake", &snap, None, None, None,
            )?;
            Ok(bundle.window)
        }));
    }

    // Promoters chase whatever is promotable, repeatedly. Refusals are
    // expected and are the point: a stale promote must lose, not corrupt.
    let mut promoters = Vec::new();
    for i in 0..PROMOTERS {
        let client = cluster.client(PUBLISHERS + i);
        promoters.push(std::thread::spawn(move || -> Result<Vec<String>> {
            let mut promoted = Vec::new();
            for _ in 0..12 {
                let report = match client.inbox("repo", "scope", None) {
                    Ok(report) => report,
                    Err(_) => continue,
                };
                for bundle in report.bundles {
                    if client
                        .promote(&bundle.bundle_id, "repo", "scope", "main")
                        .is_ok()
                    {
                        promoted.push(bundle.bundle_id);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok(promoted)
        }));
    }

    let mut windows = Vec::new();
    for handle in publishers {
        windows.push(handle.join().expect("publisher thread")?);
    }
    let mut promoted = Vec::new();
    for handle in promoters {
        promoted.extend(handle.join().expect("promoter thread")?);
    }

    // Every publication landed exactly once: window ends are 1..=N with
    // no duplicates, whatever the floor was doing underneath.
    let mut ends: Vec<u64> = windows.iter().map(|(_, end)| *end).collect();
    ends.sort_unstable();
    assert_eq!(
        ends,
        (1..=PUBLISHERS as u64).collect::<Vec<_>>(),
        "each publish committed a distinct window end: {windows:?}"
    );

    // A window never reaches back below a floor a promotion already set:
    // for each bundle, start <= end, and starts are non-decreasing in end
    // order, which is what "the floor only moves forward" looks like from
    // the outside.
    let mut by_end = windows.clone();
    by_end.sort_by_key(|(_, end)| *end);
    let mut floor_seen = 0;
    for (start, end) in by_end {
        assert!(start <= end, "window {start}..{end} is inverted");
        assert!(
            start >= floor_seen,
            "window start went backwards: {start} after seeing {floor_seen}"
        );
        floor_seen = start;
    }

    // Promotions that succeeded are distinct: the same bundle cannot be
    // promoted twice into the same gate (13.2's monotonicity guard).
    let mut unique = promoted.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        promoted.len(),
        "a bundle was promoted twice: {promoted:?}"
    );
    Ok(())
}

/// Two threads promoting the *same* bundle into the same gate at the
/// same instant. Both may report success — promote is idempotent, which
/// is what a client retrying a timed-out request needs — but the
/// partition must end up in the state one promotion would have produced,
/// and the promotion must be recorded once.
#[test]
fn simultaneous_promotion_of_one_bundle_is_idempotent() -> Result<()> {
    let cluster = Cluster::start(2)?;
    let (_dir, ws, snap) = snap_with("a.txt", "one")?;
    let (bundle, _) = cluster.client(0).publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    assert_eq!(bundle.status, BundleStatus::Ready { promotable: true });

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for i in 0..2 {
        let client = cluster.client(i);
        let barrier = Arc::clone(&barrier);
        let bundle_id = bundle.bundle_id.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            client.promote(&bundle_id, "repo", "scope", "main").is_ok()
        }));
    }
    let wins = handles
        .into_iter()
        .map(|h| h.join().expect("promoter thread"))
        .filter(|ok| *ok)
        .count();
    assert!(wins >= 1, "at least one promotion must succeed");

    // The floor advanced exactly once: the next publish opens a window
    // starting immediately after the promoted bundle's, not two past it.
    let (_dir, ws2, snap2) = snap_with("b.txt", "two")?;
    let (next, _) = cluster.client(0).publish(
        &ws2.store,
        "repo",
        "scope",
        "intake",
        &snap2,
        Some(bundle.bundle_id.clone()),
        None,
        None,
    )?;
    assert_eq!(
        next.window.0,
        bundle.window.1 + 1,
        "double promotion moved the floor twice"
    );

    // And promoting again, sequentially, is still accepted: a client
    // whose request timed out must be able to retry without a special
    // case telling it the promotion "already happened".
    assert!(
        cluster
            .client(0)
            .promote(&bundle.bundle_id, "repo", "scope", "main")
            .is_ok(),
        "promote must be idempotent for retries"
    );
    Ok(())
}

/// GC running while uploads are in flight. Batch 12.2 pinned in-flight
/// uploads; this proves the pin holds when the two genuinely overlap
/// rather than being interleaved by hand.
#[test]
fn gc_running_against_live_uploads_collects_nothing_reachable() -> Result<()> {
    const PUBLISHERS: usize = 4;
    let cluster = Cluster::start(PUBLISHERS + 1)?;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let collector = {
        let client = cluster.client(PUBLISHERS);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut runs = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                // Refusals from the single-flight guard (batch 14.4) are
                // fine; what matters is that a run that *does* happen
                // deletes nothing live.
                let _ = client.gc("repo", false);
                runs += 1;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            runs
        })
    };

    let mut publishers = Vec::new();
    for i in 0..PUBLISHERS {
        let client = cluster.client(i);
        publishers.push(std::thread::spawn(move || -> Result<String> {
            // A chunked file, so the upload spans several objects and has
            // a real window in which to be collected.
            let body: String = std::iter::repeat_n(format!("payload {i} "), 40_000).collect();
            let (_dir, ws, snap) = snap_with(&format!("big{i}.bin"), &body)?;
            let (bundle, _) = client.publish(
                &ws.store, "repo", "scope", "intake", &snap, None, None, None,
            )?;
            Ok(bundle.bundle_id)
        }));
    }

    let mut bundles = Vec::new();
    for handle in publishers {
        bundles.push(handle.join().expect("publisher thread")?);
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let runs = collector.join().expect("gc thread");
    assert!(runs > 0, "the collector never ran");

    // Every published bundle is still fully fetchable: nothing reachable
    // was collected out from under it.
    let reader = cluster.client(0);
    for bundle_id in bundles {
        let dir = tempfile::tempdir()?;
        let ws = Workspace::init(dir.path(), false)?;
        let root = reader
            .fetch_bundle(&ws.store, "repo", &bundle_id)
            .unwrap_or_else(|err| panic!("bundle {bundle_id} lost objects to GC: {err:#}"));
        let out = tempfile::tempdir()?;
        ws.materialize_manifest_to(&root, out.path(), true)
            .unwrap_or_else(|err| panic!("bundle {bundle_id} cannot materialize: {err:#}"));
    }
    Ok(())
}
