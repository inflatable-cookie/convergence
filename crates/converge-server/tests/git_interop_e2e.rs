//! g02.009 exit criterion: a real git repo imports, works under
//! Convergence (against a live server), and exports a mirror branch that
//! plain git can consume — without duplicated history.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;

use anyhow::Result;

use converge_client::git_export::export_lineage;
use converge_client::git_import::{ImportDepth, import};
use converge_client::remote::RemoteClient;
use converge_client::workspace::Workspace;
use converge_model::{BundleStatus, GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git(dir: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn start_server(data_dir: &std::path::Path) -> Result<String> {
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
                may_release: true,
            }],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "release"] {
        meta.add_grant("alice", "repo", "*", capability)?;
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

#[test]
fn git_repo_imports_works_under_convergence_and_mirrors_back() -> Result<()> {
    if !git_available() {
        eprintln!("git not available; skipping");
        return Ok(());
    }
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let alice = RemoteClient::new(&base_url, "token-a");

    // A real git repo with two commits.
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    git(root, &["init", "--quiet"])?;
    std::fs::write(root.join("game.cfg"), "resolution=1080")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "initial config"])?;
    std::fs::write(root.join("asset.bin"), vec![0u8, 159, 146, 150])?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "add binary asset"])?;

    // Import full history.
    let ws = Workspace::init(root, false)?;
    let report = import(&ws, ImportDepth::All)?;
    assert_eq!(report.imported_snaps, 2);

    // Work under Convergence: change + snap + publish + release.
    std::fs::write(root.join("game.cfg"), "resolution=1440")?;
    let snap = ws.create_snap(Some("bump resolution".into()))?;
    let (bundle, _) = alice.publish(
        &ws.store, "repo", "scope", "intake", &snap, None, None, None,
    )?;
    assert_eq!(bundle.status, BundleStatus::Ready { promotable: true });
    alice.release(&bundle.bundle_id, "repo", "scope", "stable", None)?;

    // Export the full lineage: only the new snap becomes a new commit.
    let export = export_lineage(&ws.store, root, "converge/lane/local", &snap.id)?;
    assert_eq!(export.exported_commits, 1, "imports not duplicated");
    assert_eq!(export.skipped_existing, 2);

    // Plain git consumes the mirror.
    let count = git(root, &["rev-list", "--count", "converge/lane/local"])?;
    assert_eq!(count.trim(), "3");
    let log = git(root, &["log", "--format=%s", "converge/lane/local"])?;
    assert!(log.contains("bump resolution"));
    assert!(log.contains("initial config"));
    let shown = git(root, &["show", "converge/lane/local:game.cfg"])?;
    assert_eq!(shown, "resolution=1440");

    // Nested-root guard: export from a subdirectory workspace refuses.
    let nested = root.join("nested");
    std::fs::create_dir(&nested)?;
    let nested_ws = Workspace::init(&nested, false)?;
    std::fs::write(nested.join("x.txt"), "x")?;
    let nested_snap = nested_ws.create_snap(None)?;
    let err = export_lineage(
        &nested_ws.store,
        &nested,
        "converge/lane/nested",
        &nested_snap.id,
    )
    .unwrap_err();
    let text = format!("{err:#}");
    assert!(
        text.contains("not a git repository") || text.contains("git worktree root"),
        "nested export refused: {text}"
    );
    Ok(())
}
