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
                            mode: 0o644,
                            size: 9,
                        },
                    },
                    SuperpositionVariant {
                        source: "lane-b".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: blob_b,
                            mode: 0o644,
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

    let list = json_data(&converge(root, &["--json", "resolve", "list", &snap.id]));
    assert_eq!(list["conflicted.txt"], 2);

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

    let resolved = json_data(&converge(
        root,
        &["--json", "resolve", "apply", &snap.id, "decisions.json"],
    ));
    let resolved_id = resolved.as_str().unwrap();
    let m = ws
        .store
        .get_manifest(&converge_client::model::ObjectId(resolved_id.to_string()))?;
    match &m.entries[0].kind {
        ManifestEntryKind::File { blob, .. } => {
            assert_eq!(ws.store.get_blob(blob)?, b"variant b");
        }
        other => panic!("expected resolved file, got {other:?}"),
    }
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
