//! Batch 18.2: interruptions, not happy paths.
//!
//! The audit found zero failure injection. Every durability claim the
//! system makes — torn uploads heal, GC never eats a live object,
//! corruption is caught on read — was argued rather than exercised. These
//! tests break things on purpose and assert the claims hold.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{GateGraph, GateNode, ObjectId};
use converge_server::{
    AppState, Capability, Engine, FsObjectStore, MetadataStore, ObjectKind, ObjectStore,
    SqliteMetadataStore, authorize, router,
};

fn seed_meta(dir: &std::path::Path) -> Result<SqliteMetadataStore> {
    let meta = SqliteMetadataStore::open(&dir.join("meta.sqlite"))?;
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
                may_release: true,
            }],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "resolve", "approve", "promote", "admin"] {
        meta.add_grant("alice", "repo", "*", capability)?;
    }
    Ok(meta)
}

fn serve(data_dir: &std::path::Path) -> Result<String> {
    let meta = seed_meta(data_dir)?;
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([("token-a".to_string(), "alice".to_string())]),
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

/// A TCP proxy that severs the first `cut_after` connections partway
/// through, then forwards cleanly. Closer to reality than an injected
/// error: the client sees a real half-written stream.
fn flaky_proxy(upstream: String, cut_after_bytes: usize, cuts: usize) -> Result<String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let target = upstream
        .trim_start_matches("http://")
        .parse::<std::net::SocketAddr>()?;
    let remaining_cuts = Arc::new(AtomicUsize::new(cuts));

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut client) = stream else { continue };
            let Ok(mut server) = std::net::TcpStream::connect(target) else {
                continue;
            };
            let cut = remaining_cuts
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    n.checked_sub(1).or(Some(0))
                })
                .map(|n| n > 0)
                .unwrap_or(false);
            let remaining = Arc::clone(&remaining_cuts);
            std::thread::spawn(move || {
                let _ = remaining;
                let mut upstream_reader = server.try_clone().expect("clone stream");
                let mut client_writer = client.try_clone().expect("clone stream");
                // Server -> client, unmodified.
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut upstream_reader, &mut client_writer);
                });
                // Client -> server, cut short on the marked connections.
                let mut sent = 0usize;
                let mut buf = [0u8; 8192];
                while let Ok(read) = client.read(&mut buf) {
                    if read == 0 {
                        break;
                    }
                    if cut && sent + read > cut_after_bytes {
                        let allowed = cut_after_bytes.saturating_sub(sent);
                        let _ = server.write_all(&buf[..allowed]);
                        let _ = server.flush();
                        let _ = server.shutdown(std::net::Shutdown::Both);
                        let _ = client.shutdown(std::net::Shutdown::Both);
                        return;
                    }
                    if server.write_all(&buf[..read]).is_err() {
                        break;
                    }
                    sent += read;
                }
            });
        }
    });
    Ok(format!("http://{addr}"))
}

fn workspace_with_payload(bytes: usize) -> Result<(tempfile::TempDir, Workspace)> {
    let dir = tempfile::tempdir()?;
    let ws = Workspace::init(dir.path(), false)?;
    let body: Vec<u8> = (0..bytes).map(|i| (i % 251) as u8).collect();
    std::fs::write(dir.path().join("payload.bin"), &body)?;
    std::fs::write(dir.path().join("small.txt"), "hello")?;
    Ok((dir, ws))
}

/// A connection dropped mid-upload must cost nothing but the retry: the
/// resumed publish completes and the server's tree is whole.
#[test]
fn upload_severed_mid_batch_resumes_cleanly() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let direct = serve(server_dir.path())?;
    // Cut the first two client->server streams after 64 KiB, so a batch
    // is genuinely half-written before the socket dies.
    let through_proxy = flaky_proxy(direct.clone(), 64 * 1024, 2)?;

    let (_dir, ws) = workspace_with_payload(3 * 1024 * 1024)?;
    let snap = ws.create_snap(Some("payload".into()))?;

    let flaky = RemoteClient::new(&through_proxy, "token-a");
    let mut failures = 0;
    let mut published = None;
    for _ in 0..6 {
        match flaky.publish(
            &ws.store, "repo", "scope", "intake", &snap, None, None, None,
        ) {
            Ok((bundle, _)) => {
                published = Some(bundle);
                break;
            }
            Err(_) => failures += 1,
        }
    }
    assert!(failures > 0, "the proxy never actually cut a connection");
    let bundle = published.expect("a retry eventually succeeded");

    // The tree on the server is complete: fetch it into a fresh store and
    // materialize, which reads every object and verifies each hash.
    let reader = RemoteClient::new(&direct, "token-a");
    let out_dir = tempfile::tempdir()?;
    let out_ws = Workspace::init(out_dir.path(), false)?;
    let root = reader.fetch_bundle(&out_ws.store, "repo", &bundle.bundle_id)?;
    let materialized = tempfile::tempdir()?;
    out_ws.materialize_manifest_to(&root, materialized.path(), true)?;
    assert_eq!(
        std::fs::read(materialized.path().join("payload.bin"))?.len(),
        3 * 1024 * 1024,
        "the resumed upload produced a whole tree"
    );
    Ok(())
}

/// An object corrupted on the server must surface as a loud error on the
/// client, never as content. Both stores hash on read; this proves the
/// two ends actually meet.
#[test]
fn corrupted_server_object_is_caught_before_it_reaches_a_workspace() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = serve(server_dir.path())?;
    let (_dir, ws) = workspace_with_payload(4096)?;
    let snap = ws.create_snap(Some("payload".into()))?;
    let client = RemoteClient::new(&base_url, "token-a");
    let (bundle, _) = client.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;

    // Rot one stored blob in place, the way a bad disk would.
    let blobs = server_dir.path().join("objects/blobs");
    let victim = walk_files(&blobs)
        .into_iter()
        .max_by_key(|path| path.metadata().map(|m| m.len()).unwrap_or(0))
        .expect("a stored blob");
    let mut rotten = std::fs::read(&victim)?;
    rotten[0] ^= 0xff;
    std::fs::write(&victim, &rotten)?;

    let out_dir = tempfile::tempdir()?;
    let out_ws = Workspace::init(out_dir.path(), false)?;
    let err = client
        .fetch_bundle(&out_ws.store, "repo", &bundle.bundle_id)
        .expect_err("corruption must not be served as content");
    let message = format!("{err:#}").to_lowercase();
    assert!(
        message.contains("integrity") && message.contains("corrupt"),
        "the error must name corruption, not report the object as missing: {message}"
    );
    assert!(
        !message.contains("404"),
        "a corrupt object is a server fault, not a 404: {message}"
    );
    Ok(())
}

fn walk_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// An object store that fails the Nth delete, so a GC sweep dies partway
/// through with objects still to collect.
struct FailingDeletes {
    inner: FsObjectStore,
    deletes: AtomicUsize,
    fail_at: usize,
}

impl ObjectStore for FailingDeletes {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        self.inner.put(kind, bytes)
    }
    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        self.inner.put_bytes(kind, id, bytes)
    }
    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        self.inner.get(kind, id)
    }
    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.inner.has(kind, id)
    }
    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>> {
        self.inner.list(kind)
    }
    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()> {
        if self.deletes.fetch_add(1, Ordering::SeqCst) == self.fail_at {
            anyhow::bail!("injected storage failure during sweep");
        }
        self.inner.delete(kind, id)
    }
}

/// GC dying mid-sweep is the dangerous case: a half-finished collection
/// must not have taken anything live with it, and the next run must be
/// able to finish the job.
#[test]
fn gc_interrupted_mid_sweep_keeps_live_objects_and_finishes_later() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = Arc::new(seed_meta(dir.path())?);
    let real = FsObjectStore::new(dir.path());

    // One published tree (live) plus loose garbage older than the grace
    // period (collectable).
    let (_ws_dir, ws) = workspace_with_payload(2048)?;
    let snap = ws.create_snap(Some("live".into()))?;
    let base_url = {
        let state = AppState {
            meta: Arc::clone(&meta) as Arc<dyn MetadataStore>,
            objects: Arc::new(FsObjectStore::new(dir.path())),
            tokens: HashMap::from([("token-a".to_string(), "alice".to_string())]),
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
        format!("http://{addr}")
    };
    let client = RemoteClient::new(&base_url, "token-a");
    let (bundle, _) = client.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    let live_root = bundle.root_manifest.clone().expect("bundle has a root");

    let mut garbage = Vec::new();
    for i in 0..6 {
        garbage.push(real.put(ObjectKind::Blob, format!("garbage {i}").as_bytes())?);
    }

    // Sweep with the third delete poisoned.
    let failing = Arc::new(FailingDeletes {
        inner: FsObjectStore::new(dir.path()),
        deletes: AtomicUsize::new(0),
        fail_at: 2,
    });
    let engine = Engine {
        meta: meta.as_ref(),
        objects: failing.as_ref(),
    };
    let authz = authorize(meta.as_ref(), "alice", "repo", "*", Capability::Admin)?;
    let later = "2030-01-01T00:00:00Z";
    let interrupted = engine.gc(&authz, false, later, std::time::Duration::from_secs(0));
    assert!(
        interrupted.is_err(),
        "the injected failure should surface, not be swallowed"
    );

    // Whatever it managed to delete, the live tree is untouched.
    let good = FsObjectStore::new(dir.path());
    assert!(
        good.has(ObjectKind::Manifest, &live_root),
        "GC deleted the live root manifest"
    );
    let manifest =
        converge_model::encoding::decode_manifest(&good.get(ObjectKind::Manifest, &live_root)?)?;
    for entry in &manifest.entries {
        if let converge_model::ManifestEntryKind::File { blob, .. } = &entry.kind {
            assert!(
                good.has(ObjectKind::Blob, blob),
                "GC deleted a live blob for {}",
                entry.name
            );
        }
    }

    // A clean run afterwards finishes the collection.
    let clean_objects = FsObjectStore::new(dir.path());
    let clean = Engine {
        meta: meta.as_ref(),
        objects: &clean_objects,
    };
    let report = clean.gc(&authz, false, later, std::time::Duration::from_secs(0))?;
    assert!(
        report.swept_objects > 0,
        "the second run had nothing to finish (swept {} objects)",
        report.swept_objects
    );
    for id in &garbage {
        assert!(
            !good.has(ObjectKind::Blob, id),
            "garbage survived a completed sweep"
        );
    }
    assert!(
        good.has(ObjectKind::Manifest, &live_root),
        "the completed sweep took the live root"
    );
    Ok(())
}
