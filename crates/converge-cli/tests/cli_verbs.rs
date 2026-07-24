use std::path::Path;
use std::process::{Command, Output};

use converge_client::model::{
    Manifest, ManifestEntry, ManifestEntryKind, SnapStats, SuperpositionVariant,
    SuperpositionVariantKind, compute_snap_id,
};
use converge_client::workspace::Workspace;

fn converge(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run converge")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn json_data(out: &Output) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(stdout(out).trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

#[test]
fn init_snap_history_restore_diff_roundtrip() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();

    assert!(converge(root, &["init"]).status.success());

    std::fs::write(root.join("a.txt"), "one")?;
    let snap1 = json_data(&converge(root, &["--json", "snap", "-m", "first"]));
    let id1 = snap1["id"].as_str().unwrap().to_string();

    std::fs::write(root.join("a.txt"), "two")?;
    std::fs::write(root.join("b.txt"), "new")?;
    let snap2 = json_data(&converge(root, &["--json", "snap", "-m", "second"]));
    let id2 = snap2["id"].as_str().unwrap().to_string();

    let history = json_data(&converge(root, &["--json", "history"]));
    let ids: Vec<&str> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&id1.as_str()) && ids.contains(&id2.as_str()));

    let diff = json_data(&converge(root, &["--json", "diff", &id1, &id2]));
    let statuses: Vec<(&str, &str)> = diff
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["status"].as_str().unwrap(), l["path"].as_str().unwrap()))
        .collect();
    assert!(statuses.contains(&("Modified", "a.txt")));
    assert!(statuses.contains(&("Added", "b.txt")));

    assert!(
        converge(root, &["restore", &id1, "--force"])
            .status
            .success()
    );
    assert_eq!(std::fs::read_to_string(root.join("a.txt"))?, "one");
    assert!(!root.join("b.txt").exists());
    Ok(())
}

#[test]
fn errors_exit_one_with_json_envelope() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    let out = converge(root, &["--json", "restore", "nope"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_str(stdout(&out).trim())?;
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("nope"));

    // Outside a workspace, snap fails cleanly.
    let bare = tempfile::tempdir()?;
    let out = converge(bare.path(), &["snap"]);
    assert_eq!(out.status.code(), Some(1));

    // Usage errors exit 2 (clap).
    let out = converge(root, &["diff"]);
    assert_eq!(out.status.code(), Some(2));
    Ok(())
}

#[test]
fn resolve_list_validate_apply_over_superposition() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    // Construct a snap whose manifest holds a two-variant superposition.
    let ws = Workspace::discover(root)?;
    let blob_a = ws.store.put_blob(b"variant a")?;
    let blob_b = ws.store.put_blob(b"variant b")?;
    let manifest = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "conflicted.txt".into(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![
                    SuperpositionVariant {
                        source: "lane-a".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: blob_a,
                            // Full st_mode: what a workspace scan records,
                            // so a checked-out resolution compares equal.
                            mode: 0o100644,
                            size: 9,
                        },
                    },
                    SuperpositionVariant {
                        source: "lane-b".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: blob_b,
                            mode: 0o100644,
                            size: 9,
                        },
                    },
                ],
            },
        }],
    };
    let root_manifest = ws.store.put_manifest(&manifest)?;
    let created_at = "2026-07-23T00:00:00Z".to_string();
    let snap = converge_client::model::SnapRecord {
        version: 2,
        id: compute_snap_id(&root_manifest, &[], None),
        created_at,
        root_manifest,
        parents: Vec::new(),
        derived_from_bundle: None,
        message: Some("superposed".into()),
        trigger: "explicit".into(),
        stats: SnapStats::default(),
    };
    ws.store.put_snap(&snap)?;
    ws.store.set_head(Some(&snap.id))?;

    let list = json_data(&converge(root, &["--json", "resolve", "list", &snap.id]));
    let keys = list["conflicted.txt"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["source"], "lane-a", "stable keys carry provenance");

    // Key-based decision resolves independent of variant order.
    std::fs::write(
        root.join("key-decision.json"),
        serde_json::to_vec(&serde_json::json!({"conflicted.txt": keys[1]}))?,
    )?;
    let out = converge(
        root,
        &[
            "--json",
            "resolve",
            "validate",
            &snap.id,
            "key-decision.json",
        ],
    );
    assert!(out.status.success(), "variant-key decision validates");

    // Missing decision -> invalid, exit 1.
    std::fs::write(root.join("empty.json"), "{}")?;
    let out = converge(
        root,
        &["--json", "resolve", "validate", &snap.id, "empty.json"],
    );
    assert_eq!(out.status.code(), Some(1));

    // Index decision resolves variant 1.
    std::fs::write(root.join("decisions.json"), r#"{"conflicted.txt": 1}"#)?;
    let out = converge(
        root,
        &["--json", "resolve", "validate", &snap.id, "decisions.json"],
    );
    assert!(out.status.success());

    // `--no-checkout` records the snap without touching the working tree
    // or head — the escape hatch for "resolve now, look at it later".
    let recorded = json_data(&converge(
        root,
        &[
            "--json",
            "resolve",
            "apply",
            &snap.id,
            "decisions.json",
            "--no-checkout",
        ],
    ));
    assert_eq!(recorded["checked_out"], false);
    assert_eq!(
        ws.store.get_head()?.as_deref(),
        Some(snap.id.as_str()),
        "no-checkout leaves head where the workspace still is"
    );
    assert!(
        !root.join("conflicted.txt").exists(),
        "working tree untouched"
    );

    // Applying lands a snap and checks it out (batch 16.1, audit P1.1):
    // the resolved tree used to be an orphan manifest id no verb took.
    // `--force` because this workspace holds the decisions files, which
    // are not part of any snap — checkout is a restore and says so.
    let resolved = json_data(&converge(
        root,
        &[
            "--json",
            "resolve",
            "apply",
            &snap.id,
            "decisions.json",
            "--force",
        ],
    ));
    let resolved_id = resolved["root_manifest"].as_str().unwrap();
    let m = ws
        .store
        .get_manifest(&converge_client::model::ObjectId(resolved_id.to_string()))?;
    match &m.entries[0].kind {
        ManifestEntryKind::File { blob, .. } => {
            assert_eq!(ws.store.get_blob(blob)?, b"variant b");
        }
        other => panic!("expected resolved file, got {other:?}"),
    }

    // The snap exists, is the head, carries the superposed snap as parent,
    // and the working tree holds the resolved content.
    assert_eq!(resolved["paths_resolved"], 1);
    assert_eq!(resolved["checked_out"], true);
    let resolved_snap_id = resolved["snap"].as_str().unwrap().to_string();
    assert_eq!(
        resolved["next"],
        format!("publish --snap {resolved_snap_id}"),
        "the next verb is named, not inferred"
    );
    let resolved_snap = ws.store.get_snap(&resolved_snap_id)?;
    assert_eq!(resolved_snap.parents, vec![snap.id.clone()]);
    assert_eq!(resolved_snap.stats.files, 1);
    assert_eq!(
        ws.store.get_head()?.as_deref(),
        Some(resolved_snap_id.as_str())
    );
    assert_eq!(
        std::fs::read_to_string(root.join("conflicted.txt"))?,
        "variant b"
    );

    // Status agrees: the checked-out tree is the head snap's tree.
    let status = json_data(&converge(root, &["--json", "status"]));
    assert_eq!(status["pending"]["count"], 0, "no phantom pending changes");

    // Checkout is a full materialize: the workspace now *is* the resolved
    // tree, so the scratch decisions files are gone. Same contract as
    // `restore`, which is why it needed --force.
    assert!(
        !root.join("decisions.json").exists(),
        "checkout materializes the resolved tree, nothing beside it"
    );

    // Same decisions from the same head yield the same snap id: identity
    // is content + lineage (doc 17 §1), so applying twice records one
    // node, not two. The second run is what moved head and the tree.
    assert_eq!(recorded["snap"], resolved["snap"]);
    Ok(())
}

#[test]
fn watch_once_captures_automatic_snap_only_when_changed() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    std::fs::write(root.join("w.txt"), "v1")?;
    let captures = json_data(&converge(root, &["--json", "watch", "--once"]));
    assert_eq!(captures.as_array().unwrap().len(), 1, "change captured");

    // History shows the automatic trigger.
    let history = json_data(&converge(root, &["--json", "history"]));
    assert_eq!(history[0]["trigger"], "automatic");

    // Quiet workspace: no capture.
    let captures = json_data(&converge(root, &["--json", "watch", "--once"]));
    assert_eq!(
        captures.as_array().unwrap().len(),
        0,
        "quiet tree captures nothing"
    );
    Ok(())
}

#[test]
fn status_reports_workspace_in_one_call() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    std::fs::write(root.join("a.txt"), "one")?;
    let snap = json_data(&converge(root, &["--json", "snap", "-m", "first"]));
    std::fs::write(root.join("b.txt"), "new")?;

    let status = json_data(&converge(root, &["--json", "status"]));
    assert_eq!(status["pending"]["count"], 1);
    assert_eq!(status["head"]["id"], snap["id"]);
    assert_eq!(status["head"]["trigger"], "explicit");
    assert_eq!(status["snaps"]["total"], 1);
    assert_eq!(status["snaps"]["explicit"], 1);
    assert_eq!(status["remote"]["configured"], false);
    Ok(())
}

#[test]
fn annotate_edits_message_without_changing_identity() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());
    std::fs::write(root.join("a.txt"), "x")?;
    let snap = json_data(&converge(root, &["--json", "snap"]));
    let id = snap["id"].as_str().unwrap();

    assert!(
        converge(root, &["annotate", id, "added later"])
            .status
            .success()
    );
    let history = json_data(&converge(root, &["--json", "history"]));
    assert_eq!(history[0]["id"], id, "identity unchanged");
    assert_eq!(history[0]["message"], "added later");
    Ok(())
}

/// Batch 16.2 (audit P4.19): undo the capture, keep the work.
#[test]
fn unsnap_undoes_the_capture_and_leaves_the_tree() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    std::fs::write(root.join("a.txt"), "one")?;
    let first = json_data(&converge(root, &["--json", "snap", "-m", "first"]));
    std::fs::write(root.join("a.txt"), "two")?;
    let second = json_data(&converge(root, &["--json", "snap", "-m", "second"]));

    let undone = json_data(&converge(root, &["--json", "unsnap"]));
    assert_eq!(undone["removed"], second["id"]);
    assert_eq!(undone["head"], first["id"]);
    assert_eq!(undone["record_deleted"], true);

    // The work survives as pending changes — undo removed the capture,
    // not the content.
    assert_eq!(std::fs::read_to_string(root.join("a.txt"))?, "two");
    let status = json_data(&converge(root, &["--json", "status"]));
    assert_eq!(status["head"]["id"], first["id"]);
    assert_eq!(status["pending"]["count"], 1);

    // History no longer carries the undone snap.
    let history = json_data(&converge(root, &["--json", "history"]));
    assert!(
        !history
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == second["id"]),
        "undone snap is gone from history"
    );

    // Undoing the first capture leaves no head; `--keep` retains the record.
    let undone = json_data(&converge(root, &["--json", "unsnap", "--keep"]));
    assert_eq!(undone["removed"], first["id"]);
    assert!(undone["head"].is_null());
    assert_eq!(undone["record_deleted"], false);

    // Nothing left to undo.
    let out = converge(root, &["--json", "unsnap"]);
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

/// Batch 16.2 (audit P4.18): read-only browsing of a captured tree.
#[test]
fn show_lists_a_snap_tree_without_touching_the_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    std::fs::create_dir(root.join("src"))?;
    std::fs::write(root.join("src/lib.rs"), "fn main() {}")?;
    std::fs::write(root.join("top.txt"), "hello")?;
    let snap = json_data(&converge(root, &["--json", "snap", "-m", "shown"]));
    let snap_id = snap["id"].as_str().unwrap();

    let shown = json_data(&converge(root, &["--json", "show", snap_id]));
    assert_eq!(shown["kind"], "snap");
    assert_eq!(shown["message"], "shown");
    let names: Vec<&str> = shown["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["src/", "top.txt"], "dirs are marked as such");

    // Descend into a directory.
    let nested = json_data(&converge(
        root,
        &["--json", "show", snap_id, "--path", "src"],
    ));
    assert_eq!(nested["entries"][0]["name"], "lib.rs");
    assert_eq!(nested["entries"][0]["kind"], "file");

    // A path that is not a directory is a clear error, not a panic.
    let out = converge(root, &["--json", "show", snap_id, "--path", "top.txt"]);
    assert_eq!(out.status.code(), Some(1));

    // Browsing changed nothing.
    let status = json_data(&converge(root, &["--json", "status"]));
    assert_eq!(status["pending"]["count"], 0);
    Ok(())
}
