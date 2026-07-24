//! Batch 16.1: the conflicts-as-data flow, driven the way a user drives
//! it — the real binary, a real server, no library shortcuts.
//!
//! The audit's P1.1/P1.2 finding was that this loop had no end: `resolve
//! apply` produced a manifest id no verb accepted, and the inbox's
//! "resolve" recommendation pointed at `fetch`, which cannot resolve
//! anything. This test fails if either dead end returns.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_model::{GateGraph, GateNode};
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

fn start_server(data_dir: &Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
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
                may_release: false,
            }],
        },
    )?;
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish", "resolve", "approve", "promote"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }
    for (lane, owner) in [("lane-a", "alice"), ("lane-b", "bob")] {
        meta.create_lane(&converge_model::LaneRecord {
            lane_id: lane.into(),
            repo_id: "repo".into(),
            owner: owner.into(),
            members: vec![],
            visibility: "repo".into(),
            created_at: "2026-07-25T00:00:00Z".into(),
        })?;
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

fn login(dir: &Path, base_url: &str, token: &str, lane_note: &str) {
    let out = converge(
        dir,
        &[
            "login", "--url", base_url, "--token", token, "--repo", "repo", "--scope", "scope",
            "--gate", "intake",
        ],
    );
    assert!(out.status.success(), "login failed for {lane_note}");
}

#[test]
fn conflict_to_resolved_publish_without_out_of_band_knowledge() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    // Alice and Bob change the same path in their own workspaces.
    let a_dir = tempfile::tempdir()?;
    let a = a_dir.path();
    assert!(converge(a, &["init"]).status.success());
    login(a, &base_url, "token-a", "alice");
    std::fs::write(a.join("shared.txt"), "alice version")?;
    converge(a, &["snap", "-m", "alice"]);
    assert!(
        converge(a, &["publish", "--lane", "lane-a"])
            .status
            .success()
    );

    let b_dir = tempfile::tempdir()?;
    let b = b_dir.path();
    assert!(converge(b, &["init"]).status.success());
    login(b, &base_url, "token-b", "bob");
    std::fs::write(b.join("shared.txt"), "bob version")?;
    converge(b, &["snap", "-m", "bob"]);
    assert!(
        converge(b, &["publish", "--lane", "lane-b"])
            .status
            .success()
    );

    // Alice's inbox names the superposed bundle and hands her a command.
    let inbox = json_data(&converge(a, &["--json", "inbox"]));
    let actions = converge_cli::inbox_actions(&inbox);
    let resolve_action = actions
        .iter()
        .find(|action| {
            action
                .argv
                .as_ref()
                .is_some_and(|argv| argv.first().map(String::as_str) == Some("resolve"))
        })
        .expect("inbox recommends resolving the superposed bundle");
    let argv = resolve_action.argv.clone().expect("runnable");
    assert_eq!(argv[..2], ["resolve".to_string(), "list".to_string()]);
    let bundle_id = argv[2].clone();

    // The recommendation runs as written — against a *bundle* id, with
    // its tree fetched on demand. This is the dead end audit P1.2 found.
    let listed = json_data(&converge(a, &["--json", "resolve", "list", &bundle_id]));
    let variants = listed["shared.txt"]
        .as_array()
        .expect("shared.txt is superposed");
    assert_eq!(variants.len(), 2);

    // Decide, then apply. The resolution lands as a snap in the
    // workspace, not an orphan manifest id (audit P1.1).
    std::fs::write(
        a.join("decisions.json"),
        serde_json::to_vec(&serde_json::json!({ "shared.txt": variants[0] }))?,
    )?;
    let applied = json_data(&converge(
        a,
        &[
            "--json",
            "resolve",
            "apply",
            &bundle_id,
            "decisions.json",
            "--force",
        ],
    ));
    let resolved_snap = applied["snap"].as_str().expect("snap id").to_string();
    assert_eq!(
        applied["derived_from_bundle"].as_str(),
        Some(bundle_id.as_str()),
        "provenance edge to the bundle, not a parent (doc 17 §1)"
    );
    assert_eq!(applied["checked_out"], true);
    assert_eq!(
        std::fs::read_to_string(a.join("shared.txt"))?,
        "alice version",
        "workspace holds the resolved tree"
    );

    // The `next` field is a real command: run it verbatim.
    let next: Vec<&str> = applied["next"]
        .as_str()
        .expect("next verb")
        .split(' ')
        .collect();
    let mut publish_argv = vec!["--json"];
    publish_argv.extend(next);
    let published = json_data(&converge(a, &publish_argv));
    assert_eq!(
        published["bundle"]["status"],
        serde_json::json!({ "ready": { "promotable": true } }),
        "the resolved publish is promotable — the superposition is gone"
    );

    // History shows the resolution as a first-class snap.
    let history = json_data(&converge(a, &["--json", "history"]));
    assert!(
        history
            .as_array()
            .expect("history list")
            .iter()
            .any(|s| s["id"] == resolved_snap.as_str()),
        "the resolution snap is in history"
    );
    Ok(())
}
