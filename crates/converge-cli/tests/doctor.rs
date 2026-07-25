//! Batch 22.1: `converge doctor` answers "what is wrong with this
//! setup", and answers all of it.
//!
//! The property worth pinning is that it does not stop at the first
//! failure. Stopping there would reproduce exactly the problem the verb
//! exists to solve: every verb already reports its own failure, and the
//! one you hit first is rarely the first thing that is wrong.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", home)
        .args(args)
        .output()
        .expect("run converge")
}

fn report(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<serde_json::Value>(text.trim())
        .map(|v| v["data"].clone())
        .unwrap_or_default()
}

fn check<'a>(report: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no `{name}` check in {report}"))
}

fn start_server(data_dir: &Path) -> Result<(String, String)> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("acme")?;
    meta.create_scope("acme", "default", "2026-07-25T00:00:00Z")?;
    meta.upsert_user("dana")?;
    meta.add_grant("dana", "acme", "*", "read")?;
    let token = converge_server::mint_admin_token()?;
    meta.create_token(&converge_server::token_hash(&token), "dana")?;

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::new(),
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
    Ok((format!("http://{addr}"), token))
}

/// The whole point: several things are wrong, and it says so about all
/// of them.
#[test]
fn a_broken_setup_gets_every_problem_and_every_fix() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;

    let out = converge(dir.path(), home.path(), &["--json", "doctor"]);
    assert!(!out.status.success(), "a broken setup should exit non-zero");
    let report = report(&out);
    assert_eq!(report["ok"], false);

    // No workspace *and* no key, not just whichever came first.
    assert_eq!(check(&report, "workspace")["ok"], false);
    assert_eq!(check(&report, "personal key")["ok"], false);

    for name in ["workspace", "personal key"] {
        let fix = check(&report, name)["fix"].as_str().unwrap_or("");
        assert!(
            fix.contains("converge "),
            "`{name}` reported a problem without naming a command: {fix:?}"
        );
    }
    Ok(())
}

#[test]
fn a_healthy_setup_says_so_and_exits_zero() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, token) = start_server(server_dir.path())?;
    let dir = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;

    assert!(
        converge(dir.path(), home.path(), &["init"])
            .status
            .success()
    );
    assert!(
        converge(
            dir.path(),
            home.path(),
            &[
                "login", "--url", &base_url, "--token", &token, "--repo", "acme", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );
    // A key exists, so no check is failing for a reason unrelated to the
    // server round trip.
    assert!(
        Command::new(env!("CARGO_BIN_EXE_converge"))
            .current_dir(dir.path())
            .env("CONVERGE_HOME", home.path())
            .env("CONVERGE_PASSPHRASE", "test-passphrase")
            .args(["key", "init", "--yes"])
            .output()?
            .status
            .success()
    );

    let out = converge(dir.path(), home.path(), &["--json", "doctor"]);
    let report = report(&out);
    assert!(
        out.status.success(),
        "a working setup should exit zero: {report}"
    );
    assert_eq!(report["ok"], true);
    assert_eq!(check(&report, "server")["ok"], true);
    // Skew is measured against the server's own clock, and against
    // itself that is zero.
    assert_eq!(check(&report, "clock")["ok"], true);
    Ok(())
}

/// 401 and 403 are different problems with different fixes, and batch
/// 21.4 made the server say which. `doctor` is where somebody finally
/// reads that without guessing which verb to run.
#[test]
fn an_unusable_credential_is_told_apart_from_a_missing_grant() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, token) = start_server(server_dir.path())?;

    // A token the server never issued: authentication.
    let dir = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;
    converge(dir.path(), home.path(), &["init"]);
    converge(
        dir.path(),
        home.path(),
        &[
            "login",
            "--url",
            &base_url,
            "--token",
            "not-a-real-token",
            "--repo",
            "acme",
            "--scope",
            "default",
            "--gate",
            "intake",
        ],
    );
    let bad_token = report(&converge(dir.path(), home.path(), &["--json", "doctor"]));
    let credential = check(&bad_token, "credential");
    assert_eq!(credential["ok"], false);
    assert!(
        credential["fix"].as_str().unwrap_or("").contains("login"),
        "a bad credential is fixed by logging in again: {credential}"
    );

    // A real token for a subject with no grants in another repo:
    // authorization. The fix names both possibilities on purpose —
    // a repo that does not exist and one you cannot read are the same
    // answer from the server, deliberately.
    let dir = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;
    converge(dir.path(), home.path(), &["init"]);
    converge(
        dir.path(),
        home.path(),
        &[
            "login",
            "--url",
            &base_url,
            "--token",
            &token,
            "--repo",
            "other-repo",
            "--scope",
            "default",
            "--gate",
            "intake",
        ],
    );
    let no_access = report(&converge(dir.path(), home.path(), &["--json", "doctor"]));
    let access = check(&no_access, "access");
    assert_eq!(access["ok"], false);
    let fix = access["fix"].as_str().unwrap_or("");
    assert!(
        fix.contains("repo create") && fix.contains("member add"),
        "the server cannot tell 'no such repo' from 'no access', so the fix \
         must name both: {fix}"
    );
    Ok(())
}

/// A diagnostic you cannot safely run when you are unsure is not one.
#[test]
fn doctor_changes_nothing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;
    converge(dir.path(), home.path(), &["init"]);

    let before: Vec<_> = walk(dir.path());
    converge(dir.path(), home.path(), &["doctor"]);
    converge(dir.path(), home.path(), &["--json", "doctor"]);
    assert_eq!(before, walk(dir.path()), "doctor wrote to the workspace");
    assert!(
        walk(home.path()).is_empty(),
        "doctor created something under CONVERGE_HOME"
    );
    Ok(())
}

fn walk(root: &Path) -> Vec<(std::path::PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => stack.push(path),
                Ok(meta) => out.push((path, meta.len())),
                Err(_) => {}
            }
        }
    }
    out.sort();
    out
}
