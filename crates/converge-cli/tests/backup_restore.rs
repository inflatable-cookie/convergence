//! Batch 22.3: a backup you have not restored is a hypothesis.
//!
//! Doc 19 §1 concedes availability in one sentence — the server holds
//! secrets it cannot read, so it cannot regenerate them, and a lost
//! object store loses them permanently. The backup is the only
//! mitigation that exists, which makes "does restoring actually work"
//! a property worth pinning rather than a paragraph in a guide.
//!
//! The server here runs in-process against a data directory, which is
//! what a real deployment is: copy the directory, point a server at the
//! copy.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", home)
        .env("CONVERGE_PASSPHRASE", "backup-test")
        .args(args)
        .output()
        .expect("run converge")
}

fn json(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<serde_json::Value>(text.trim())
        .map(|v| v["data"].clone())
        .unwrap_or_default()
}

/// Serve `data_dir` on a fresh port. Returns the base url.
fn serve(data_dir: &Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([("t".to_string(), "alice".to_string())]),
        gc_running: Default::default(),
        oidc: None,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok(format!("http://{addr}"))
}

fn provision(data_dir: &Path) -> Result<()> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("acme")?;
    meta.create_scope("acme", "default", "2026-07-26T00:00:00Z")?;
    meta.set_gate_graph(
        "acme",
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
    for capability in ["read", "publish", "release", "secret", "admin"] {
        meta.add_grant("alice", "acme", "*", capability)?;
    }
    Ok(())
}

/// Copy a directory tree. This is what a backup *is*.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.metadata()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

struct Deployment {
    _dir: tempfile::TempDir,
    data: std::path::PathBuf,
    bundle_id: String,
}

/// A deployment with published work, a release, and a secret — the three
/// things a restore has to bring back.
fn deployment(ws: &Path, home: &Path) -> Result<Deployment> {
    let dir = tempfile::tempdir()?;
    let data = dir.path().to_path_buf();
    std::fs::create_dir_all(&data)?;
    provision(&data)?;
    let base_url = serve(&data)?;

    assert!(converge(ws, home, &["init"]).status.success());
    assert!(
        converge(
            ws,
            home,
            &[
                "login", "--url", &base_url, "--token", "t", "--repo", "acme", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );
    std::fs::write(ws.join("README.md"), "acme")?;
    std::fs::create_dir_all(ws.join("docs"))?;
    std::fs::write(ws.join("docs/plan.md"), "the plan")?;
    assert!(converge(ws, home, &["snap", "-m", "initial"]).status.success());

    let published = json(&converge(ws, home, &["--json", "publish"]));
    let bundle_id = published["bundle"]["bundle_id"]
        .as_str()
        .expect("bundle id")
        .to_string();

    assert!(
        converge(ws, home, &["key", "init", "--yes"])
            .status
            .success()
    );
    let set = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(ws)
        .env("CONVERGE_HOME", home)
        .env("CONVERGE_PASSPHRASE", "backup-test")
        .args(["secret", "set", "DATABASE_URL"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(b"postgres://user:pw@db/acme")?;
            child.wait_with_output()
        })?;
    assert!(set.status.success(), "secret set failed");

    assert!(
        converge(ws, home, &["release", &bundle_id, "--channel", "stable"])
            .status
            .success()
    );

    Ok(Deployment {
        _dir: dir,
        data,
        bundle_id,
    })
}

/// The whole claim, end to end: copy the data directory, serve the copy,
/// and everything that mattered is still there — including the secret,
/// which is the thing that cannot be regenerated.
#[test]
fn a_restored_deployment_still_serves_trees_provenance_and_secrets() -> Result<()> {
    let ws_dir = tempfile::tempdir()?;
    let home_dir = tempfile::tempdir()?;
    let (ws, home) = (ws_dir.path(), home_dir.path());
    let live = deployment(ws, home)?;

    // Back up, then serve the backup as if the original were gone.
    let backup_dir = tempfile::tempdir()?;
    let backup = backup_dir.path().join("restored");
    copy_tree(&live.data, &backup)?;
    let restored_url = serve(&backup)?;

    assert!(
        converge(
            ws,
            home,
            &[
                "login", "--url", &restored_url, "--token", "t", "--repo", "acme", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );

    // The credential still opens. This is the one that cannot be
    // regenerated from anywhere else.
    let got = converge(ws, home, &["--json", "secret", "get", "DATABASE_URL"]);
    assert!(got.status.success(), "the secret did not survive the restore");
    assert_eq!(json(&got)["value"], "postgres://user:pw@db/acme");

    // Provenance still replays: the objects are there *and* consistent.
    let verified = converge(ws, home, &["--json", "verify", &live.bundle_id]);
    assert!(
        verified.status.success(),
        "verification failed after restore: {}",
        String::from_utf8_lossy(&verified.stdout)
    );

    // And the release still resolves to a materializable tree — into a
    // scratch directory, since the workspace is not the thing under test.
    let into = backup_dir.path().join("materialized");
    let fetched = converge(
        ws,
        home,
        &[
            "fetch",
            "--release",
            "stable",
            "--into",
            into.to_str().expect("utf8"),
        ],
    );
    assert!(fetched.status.success());
    assert_eq!(std::fs::read_to_string(into.join("docs/plan.md"))?, "the plan");
    Ok(())
}

/// The mistake this batch exists to catch: backing up the database and
/// not the objects. Every ordinary check passes; `doctor --deep` is the
/// one that does not.
#[test]
fn a_backup_missing_its_objects_passes_every_check_except_the_deep_one() -> Result<()> {
    let ws_dir = tempfile::tempdir()?;
    let home_dir = tempfile::tempdir()?;
    let (ws, home) = (ws_dir.path(), home_dir.path());
    let live = deployment(ws, home)?;

    let backup_dir = tempfile::tempdir()?;
    let gutted = backup_dir.path().join("db-only");
    copy_tree(&live.data, &gutted)?;
    std::fs::remove_dir_all(gutted.join("objects"))?;
    let url = serve(&gutted)?;

    // A *clean* client, deliberately. A workspace that already fetched
    // is served out of its own store and proves nothing about the
    // server — batch 22.3 watched exactly that happen and report success.
    let clean_dir = tempfile::tempdir()?;
    let clean_home = tempfile::tempdir()?;
    let clean = clean_dir.path();
    assert!(converge(clean, clean_home.path(), &["init"]).status.success());
    assert!(
        converge(
            clean,
            clean_home.path(),
            &[
                "login", "--url", &url, "--token", "t", "--repo", "acme", "--scope", "default",
                "--gate", "intake",
            ],
        )
        .status
        .success()
    );

    // Plain doctor is happy: the control plane is fine, and that is all
    // it looks at.
    let shallow = converge(clean, clean_home.path(), &["--json", "doctor"]);
    let report = json(&shallow);
    let serving = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == "serving");
    assert!(serving.is_none(), "plain doctor should not do round trips");

    // Deep is not.
    let deep = converge(clean, clean_home.path(), &["--json", "doctor", "--deep"]);
    assert!(
        !deep.status.success(),
        "a deployment that cannot serve its own release reported healthy"
    );
    let report = json(&deep);
    let serving = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == "serving")
        .expect("serving check");
    assert_eq!(serving["ok"], false);
    assert!(
        serving["fix"]
            .as_str()
            .unwrap_or("")
            .contains("not just the database"),
        "the fix should name the mistake that caused it: {serving}"
    );

    // And the object store being gone is exactly what it is: verify
    // fails too, from any client.
    let verified = converge(clean, clean_home.path(), &["verify", &live.bundle_id]);
    assert!(!verified.status.success());
    Ok(())
}
