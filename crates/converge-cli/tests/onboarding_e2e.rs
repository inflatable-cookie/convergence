//! Batch 16.3 (audit P1.6): a team can be set up end to end without
//! restarting the server or editing its flags.
//!
//! The server here is started the way an operator starts it — no seeded
//! repo, no `--token` pairs beyond the bootstrap admin — so anything the
//! test needs must be reachable through the API.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run converge")
}

fn json_data(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

/// Bare server plus one bootstrapped admin — the state `converge-server
/// --bootstrap-admin root` leaves behind.
fn start_bare_server(data_dir: &Path) -> Result<(String, String)> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.upsert_user("root")?;
    meta.add_grant("root", "*", "*", "admin")?;
    let admin_token = converge_server::mint_admin_token()?;
    meta.create_token(&converge_server::token_hash(&admin_token), "root")?;

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::new(), // no startup tokens: everything is issued
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
    Ok((format!("http://{addr}"), admin_token))
}

fn login(dir: &Path, base_url: &str, token: &str, repo: &str) -> Output {
    converge(
        dir,
        &[
            "login", "--url", base_url, "--token", token, "--repo", repo, "--scope", "default",
            "--gate", "intake",
        ],
    )
}

#[test]
fn two_person_team_from_bootstrap_to_published_work() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;

    // The admin logs in naming a repo that does not exist yet — `login`
    // only writes local config, which is what makes create possible.
    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge(admin, &["init"]).status.success());
    assert!(
        login(admin, &base_url, &admin_token, "acme")
            .status
            .success()
    );

    let created = json_data(&converge(admin, &["--json", "repo", "create"]));
    assert_eq!(created["repo_id"], "acme");
    assert_eq!(created["scope"], "default");
    assert_eq!(created["gate"], "intake");

    // The admin can work in the repo immediately.
    std::fs::write(admin.join("plan.md"), "the plan")?;
    converge(admin, &["snap", "-m", "plan"]);
    assert!(converge(admin, &["publish"]).status.success());

    // Onboard a teammate and issue their token — no server restart.
    let added = json_data(&converge(
        admin,
        &[
            "--json",
            "member",
            "add",
            "dana",
            "--capability",
            "read",
            "--capability",
            "publish",
            "--issue-token",
        ],
    ));
    assert_eq!(added["subject"], "dana");
    let dana_token = added["token"]
        .as_str()
        .expect("token issued once")
        .to_string();

    // Unknown capabilities are refused rather than stored as dead rows.
    let out = converge(
        admin,
        &["--json", "member", "add", "eve", "--capability", "wizard"],
    );
    assert_eq!(out.status.code(), Some(1));

    // Dana logs in with the issued token and does real work.
    let dana_dir = tempfile::tempdir()?;
    let dana = dana_dir.path();
    assert!(converge(dana, &["init"]).status.success());
    assert!(login(dana, &base_url, &dana_token, "acme").status.success());
    std::fs::write(dana.join("dana.md"), "dana's work")?;
    converge(dana, &["snap", "-m", "dana"]);
    let published = json_data(&converge(dana, &["--json", "publish"]));
    assert_eq!(published["bundle"]["produced_by_gate_id"], "intake");

    // Membership is visible to the team.
    let members = json_data(&converge(dana, &["--json", "member", "list"]));
    let subjects: Vec<&str> = members
        .as_array()
        .expect("member list")
        .iter()
        .map(|m| m["subject"].as_str().unwrap())
        .collect();
    assert!(subjects.contains(&"root") && subjects.contains(&"dana"));

    // Dana is not an admin: no onboarding anyone, no creating repos.
    let out = converge(
        dana,
        &["--json", "member", "add", "mallory", "--issue-token"],
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "publish rights are not admin rights"
    );
    let out = converge(dana, &["--json", "repo", "create", "shadow"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "repo creation is site-admin only"
    );

    // A revoked-looking token (never issued) is simply unknown.
    let stranger_dir = tempfile::tempdir()?;
    let stranger = stranger_dir.path();
    assert!(converge(stranger, &["init"]).status.success());
    assert!(
        login(stranger, &base_url, "not-a-real-token", "acme")
            .status
            .success()
    );
    let out = converge(stranger, &["--json", "member", "list"]);
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}
