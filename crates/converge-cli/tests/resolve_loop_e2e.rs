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
        // Isolate the identity directory (batch 22.4). Without this the
        // suite writes real token files into the developer's own
        // `~/.converge` — 493 of them had accumulated before anyone
        // looked — and `machine_key()` regenerates on an unreadable
        // read, so a test run could in principle orphan every token the
        // user actually depends on.
        //
        // Outside the workspace, not inside it: an identity directory
        // under the tree being captured becomes part of the snap, which
        // breaks the very checkouts these tests assert on. One home per
        // test binary is isolation enough, since token keys already
        // include the workspace root.
        .env("CONVERGE_HOME", test_home())
        .args(args)
        .output()
        .expect("run converge")
}

/// One identity directory per test binary, outside every workspace.
fn test_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("converge-test-home-{}", std::process::id()))
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

    // Alice's inbox names the superposed candidate and hands her a command.
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
        .expect("inbox recommends resolving the superposed candidate");
    let argv = resolve_action.argv.clone().expect("runnable");
    assert_eq!(argv[..2], ["resolve".to_string(), "list".to_string()]);
    let candidate_id = argv[2].clone();

    // The recommendation runs as written — against a *candidate* id, with
    // its tree fetched on demand. This is the dead end audit P1.2 found.
    let listed = json_data(&converge(a, &["--json", "resolve", "list", &candidate_id]));
    let variants = listed["shared.txt"]
        .as_array()
        .expect("shared.txt is superposed");
    assert_eq!(variants.len(), 2);

    // `--preview` shows what the choice is *between* (batch 23.5). The
    // flat list asked people to pick variant 1 or 2 sight unseen, which
    // batch 23.1 recorded as a decision-correctness problem rather than
    // a missing nicety.
    let previewed = json_data(&converge(
        a,
        &["--json", "resolve", "list", &candidate_id, "--preview"],
    ));
    let shown: Vec<String> = previewed["shared.txt"]
        .as_array()
        .expect("previewed variants")
        .iter()
        .map(|v| v["preview"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        shown.iter().any(|p| p == "alice version") && shown.iter().any(|p| p == "bob version"),
        "both versions should be legible before choosing: {shown:?}"
    );
    // The key is still there and still the thing a decisions file wants,
    // so the preview is additive rather than a second shape.
    assert_eq!(previewed["shared.txt"][0]["key"], variants[0]);

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
            &candidate_id,
            "decisions.json",
            "--force",
        ],
    ));
    let resolved_snap = applied["snap"].as_str().expect("snap id").to_string();
    assert_eq!(
        applied["derived_from_candidate"].as_str(),
        Some(candidate_id.as_str()),
        "provenance edge to the candidate, not a parent (doc 17 §1)"
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
        published["candidate"]["status"],
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

/// Batch 16.2 (audit P1.3/P1.4): arriving at someone else's work must not
/// need out-of-band knowledge either. Pull materializes; fetch checks out.
#[test]
fn arrival_paths_land_work_in_the_workspace() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    let a_dir = tempfile::tempdir()?;
    let a = a_dir.path();
    assert!(converge(a, &["init"]).status.success());
    login(a, &base_url, "token-a", "alice");
    std::fs::write(a.join("shared.txt"), "alice version")?;
    converge(a, &["snap", "-m", "alice"]);
    assert!(
        converge(a, &["sync", "push", "--lane", "lane-a"])
            .status
            .success()
    );
    let published = json_data(&converge(a, &["--json", "publish", "--lane", "lane-a"]));
    let candidate_id = published["candidate"]["candidate_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Bob pulls the lane. Without --materialize he is told what to run
    // next; with it, the workspace is updated in one step.
    let b_dir = tempfile::tempdir()?;
    let b = b_dir.path();
    assert!(converge(b, &["init"]).status.success());
    login(b, &base_url, "token-b", "bob");

    let pulled = json_data(&converge(
        b,
        &["--json", "sync", "pull", "--lane", "lane-a"],
    ));
    assert_eq!(pulled["materialized"], false);
    assert!(
        !b.join("shared.txt").exists(),
        "pull alone does not write files"
    );
    assert_eq!(
        pulled["next"].as_str(),
        Some(format!("restore {}", pulled["head"].as_str().unwrap()).as_str()),
        "the manual step is named, not assumed"
    );

    let pulled = json_data(&converge(
        b,
        &[
            "--json",
            "sync",
            "pull",
            "--lane",
            "lane-a",
            "--materialize",
        ],
    ));
    assert_eq!(pulled["materialized"], true);
    assert_eq!(
        std::fs::read_to_string(b.join("shared.txt"))?,
        "alice version"
    );

    // Fetching a candidate with --checkout lands it as a snap to continue
    // from, with the candidate as provenance (doc 17 §1).
    let c_dir = tempfile::tempdir()?;
    let c = c_dir.path();
    assert!(converge(c, &["init"]).status.success());
    login(c, &base_url, "token-b", "bob");

    let bare = json_data(&converge(c, &["--json", "fetch", &candidate_id]));
    assert!(bare["snap"].is_null());
    assert_eq!(
        bare["next"].as_str(),
        Some(format!("show {candidate_id}").as_str())
    );
    assert!(
        !c.join("shared.txt").exists(),
        "a bare fetch writes no files"
    );

    // `show` works on the fetched candidate without materializing anything.
    let shown = json_data(&converge(c, &["--json", "show", &candidate_id]));
    assert_eq!(shown["kind"], "candidate");
    assert_eq!(shown["entries"][0]["name"], "shared.txt");

    let checked_out = json_data(&converge(
        c,
        &["--json", "fetch", &candidate_id, "--checkout"],
    ));
    let snap_id = checked_out["snap"]
        .as_str()
        .expect("checkout captures a snap");
    assert_eq!(
        std::fs::read_to_string(c.join("shared.txt"))?,
        "alice version"
    );
    let status = json_data(&converge(c, &["--json", "status"]));
    assert_eq!(status["head"]["id"], snap_id, "head follows the checkout");
    assert_eq!(status["pending"]["count"], 0);

    // --into and --checkout mean different things and refuse to be mixed.
    let out = converge(
        c,
        &[
            "--json",
            "fetch",
            &candidate_id,
            "--checkout",
            "--into",
            "copy",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

/// Batch 16.4 (audit P3, P4.20): the verbs that render *server* records
/// are where `{:?}` actually leaked, and the transfer they drive is what
/// looked hung on a large binary.
#[test]
fn remote_human_output_reads_like_prose_and_reports_transfer() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    let a_dir = tempfile::tempdir()?;
    let a = a_dir.path();
    assert!(converge(a, &["init"]).status.success());
    login(a, &base_url, "token-a", "alice");
    // Big enough to cross a transfer batch, so progress has something to
    // report rather than finishing in one flush.
    std::fs::write(a.join("big.bin"), vec![7u8; 12 * 1024 * 1024])?;
    std::fs::write(a.join("readme.md"), "hello")?;
    converge(a, &["snap", "-m", "alice"]);

    let publish = converge(a, &["publish", "--lane", "lane-a"]);
    assert!(publish.status.success());
    let out = String::from_utf8_lossy(&publish.stdout).into_owned();
    let err = String::from_utf8_lossy(&publish.stderr).into_owned();

    assert!(
        out.contains("ready to promote"),
        "status is phrased, not Debug-printed:\n{out}"
    );
    for marker in ["Ready {", "Some(", "{:?}"] {
        assert!(!out.contains(marker), "leaked {marker:?}:\n{out}");
    }
    assert!(
        err.contains("upload") && err.contains("objects"),
        "transfer progress goes to stderr:\n{err}"
    );
    assert!(
        !out.contains("upload "),
        "progress must not pollute stdout:\n{out}"
    );

    // `--json` gets the envelope on stdout and no progress chatter at all.
    std::fs::write(a.join("readme.md"), "hello again")?;
    converge(a, &["snap", "-m", "second"]);
    let published = converge(a, &["--json", "publish", "--lane", "lane-a"]);
    let out = String::from_utf8_lossy(&published.stdout).into_owned();
    assert_eq!(out.trim().lines().count(), 1, "one envelope line:\n{out}");
    assert!(
        String::from_utf8_lossy(&published.stderr).is_empty(),
        "no progress in machine mode"
    );

    let candidate_id = serde_json::from_str::<serde_json::Value>(out.trim())?["data"]["candidate"]
        ["candidate_id"]
        .as_str()
        .expect("candidate id")
        .to_string();

    // Candidate inspection: prose, and addressable the same way fetch is.
    let shown = converge(a, &["candidate", &candidate_id]);
    let text = String::from_utf8_lossy(&shown.stdout).into_owned();
    assert!(
        text.contains("publication") && !text.contains("("),
        "window renders as a range, not a tuple:\n{text}"
    );

    // Retention limits read as numbers or "keep all", never Some/None.
    let retention = converge(a, &["retention", "show"]);
    let text = String::from_utf8_lossy(&retention.stdout).into_owned();
    assert!(
        text.contains("keep all") && !text.contains("None"),
        "retention limits are phrased:\n{text}"
    );
    Ok(())
}

/// What a preview does when there is nothing readable to show.
///
/// Refusing to guess is the point: two screens of replacement characters
/// help nobody choose, while "binary" and two sizes do (batch 23.5).
#[test]
fn previews_are_bounded_and_say_why_when_there_is_no_text() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    let a_dir = tempfile::tempdir()?;
    let a = a_dir.path();
    assert!(converge(a, &["init"]).status.success());
    login(a, &base_url, "token-a", "alice");
    // One binary path and one very long text path, conflicting.
    std::fs::write(a.join("image.bin"), [0u8, 1, 2, 3, 0, 5])?;
    std::fs::write(a.join("long.txt"), "alice\n".repeat(4000))?;
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
    std::fs::write(b.join("image.bin"), [0u8, 9, 9, 9, 0, 9])?;
    std::fs::write(b.join("long.txt"), "bob\n".repeat(4000))?;
    converge(b, &["snap", "-m", "bob"]);
    assert!(
        converge(b, &["publish", "--lane", "lane-b"])
            .status
            .success()
    );

    let inbox = json_data(&converge(a, &["--json", "inbox"]));
    let candidate_id = converge_cli::inbox_actions(&inbox)
        .iter()
        .find_map(|action| {
            action
                .argv
                .as_ref()
                .filter(|argv| argv.first().map(String::as_str) == Some("resolve"))
                .map(|argv| argv[2].clone())
        })
        .expect("a superposed candidate");

    let previewed = json_data(&converge(
        a,
        &["--json", "resolve", "list", &candidate_id, "--preview"],
    ));

    for variant in previewed["image.bin"].as_array().expect("binary variants") {
        assert_eq!(
            variant["preview"].as_str(),
            Some(""),
            "a binary should have no text"
        );
        let why = variant["why"].as_str().unwrap_or("");
        assert!(
            why.starts_with("binary") && why.contains("bytes"),
            "two variants labelled only 'binary' are not a choice; the size is \
             usually what tells them apart: {why}"
        );
    }

    for variant in previewed["long.txt"].as_array().expect("long variants") {
        let preview = variant["preview"].as_str().expect("text");
        assert!(
            preview.lines().count() <= 12,
            "a preview is bounded: {} lines",
            preview.lines().count()
        );
        assert!(
            variant["elided"].as_bool().unwrap_or(false),
            "and says the content continues"
        );
    }
    Ok(())
}

/// Batch 22.4, from the first real conflict: two rewrites of the same
/// Rust file shared a nine-line header, so a preview from line 1 spent
/// its whole budget on identical text and truncated exactly where the
/// disagreement began.
#[test]
fn a_preview_skips_what_every_variant_agrees_on() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;

    // A file whose head is boilerplate and whose difference is deep.
    let header = "//! A module.\n//!\n//! Several lines of doc comment.\n\npub fn f() {\n";
    let a_dir = tempfile::tempdir()?;
    let a = a_dir.path();
    assert!(converge(a, &["init"]).status.success());
    login(a, &base_url, "token-a", "alice");
    std::fs::write(a.join("m.rs"), format!("{header}    let x = 1;\n}}\n"))?;
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
    std::fs::write(b.join("m.rs"), format!("{header}    let x = 2;\n}}\n"))?;
    converge(b, &["snap", "-m", "bob"]);
    assert!(
        converge(b, &["publish", "--lane", "lane-b"])
            .status
            .success()
    );

    let inbox = json_data(&converge(a, &["--json", "inbox"]));
    let candidate_id = converge_cli::inbox_actions(&inbox)
        .iter()
        .find_map(|action| {
            action
                .argv
                .as_ref()
                .filter(|argv| argv.first().map(String::as_str) == Some("resolve"))
                .map(|argv| argv[2].clone())
        })
        .expect("a superposed candidate");

    let previewed = json_data(&converge(
        a,
        &["--json", "resolve", "list", &candidate_id, "--preview"],
    ));
    let variants = previewed["m.rs"].as_array().expect("variants");
    assert_eq!(variants.len(), 2);

    for variant in variants {
        let skipped = variant["skipped_common_lines"].as_u64().unwrap_or(0);
        assert!(
            skipped >= 4,
            "the shared header should be skipped: {variant}"
        );
        let text = variant["preview"].as_str().unwrap_or("");
        assert!(
            !text.contains("Several lines of doc comment"),
            "the preview still leads with text every variant shares: {text}"
        );
        assert!(
            text.contains("let x ="),
            "the preview should start at the disagreement: {text}"
        );
    }

    // The two previews must actually differ, or the trim went too far.
    assert_ne!(variants[0]["preview"], variants[1]["preview"]);
    Ok(())
}
