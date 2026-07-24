//! Doc 18 §2 export: fidelity, trailers, incrementality, refusal rules.
//! Requires `git` on PATH; tests no-op gracefully without it.

use std::process::Command;

use anyhow::Result;

use converge_client::git_export::export_lineage;
use converge_client::workspace::Workspace;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_out(dir: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    anyhow::ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn export_lineage_with_fidelity_and_incrementality() -> Result<()> {
    if !git_available() {
        eprintln!("git not available; skipping");
        return Ok(());
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    git_out(root, &["init", "--quiet"])?;
    let ws = Workspace::init(root, false)?;

    std::fs::write(root.join("a.txt"), "one")?;
    ws.create_snap(Some("first".into()))?;
    std::fs::write(root.join("a.txt"), "two")?;
    std::fs::write(root.join("b.bin"), vec![0u8, 1, 2, 250])?;
    ws.create_snap(Some("second".into()))?;
    std::fs::write(root.join("c.txt"), "three")?;
    let s3 = ws.create_snap(None)?;

    let report = export_lineage(&ws.store, root, "converge/lane/local", &s3.id)?;
    assert_eq!(report.exported_commits, 3);

    // History + trailers.
    let log = git_out(root, &["log", "--format=%B", "converge/lane/local"])?;
    assert_eq!(log.matches("Converge-Snap: ").count(), 3);
    assert!(log.contains("first") && log.contains("second"));

    // Fidelity: checkout tree byte-identical to the workspace tree.
    let clone_dir = tempfile::tempdir()?;
    git_out(
        root,
        &[
            "clone",
            "--quiet",
            "--branch",
            "converge/lane/local",
            root.to_str().unwrap(),
            clone_dir.path().to_str().unwrap(),
        ],
    )?;
    for file in ["a.txt", "b.bin", "c.txt"] {
        assert_eq!(
            std::fs::read(clone_dir.path().join(file))?,
            std::fs::read(root.join(file))?,
            "{file} byte-identical"
        );
    }
    assert!(
        !clone_dir.path().join(".converge").exists(),
        "internals never exported"
    );

    // git never tracks .converge locally either.
    let status = git_out(root, &["status", "--porcelain"])?;
    assert!(!status.contains(".converge"), "info/exclude installed");

    // Incremental re-export: nothing new.
    let report = export_lineage(&ws.store, root, "converge/lane/local", &s3.id)?;
    assert_eq!(report.exported_commits, 0);
    assert_eq!(report.skipped_existing, 3);

    // New snap exports exactly one commit on top.
    std::fs::write(root.join("d.txt"), "four")?;
    let s4 = ws.create_snap(None)?;
    let report = export_lineage(&ws.store, root, "converge/lane/local", &s4.id)?;
    assert_eq!(report.exported_commits, 1);
    let count = git_out(root, &["rev-list", "--count", "converge/lane/local"])?;
    assert_eq!(count.trim(), "4");
    Ok(())
}

#[test]
fn superposed_tree_refuses_to_export() -> Result<()> {
    if !git_available() {
        return Ok(());
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    git_out(root, &["init", "--quiet"])?;
    let ws = Workspace::init(root, false)?;

    use converge_client::model::*;
    let blob_a = ws.store.put_blob(b"a")?;
    let blob_b = ws.store.put_blob(b"b")?;
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
                            size: 1,
                        },
                    },
                    SuperpositionVariant {
                        source: "lane-b".into(),
                        kind: SuperpositionVariantKind::File {
                            blob: blob_b,
                            mode: 0o644,
                            size: 1,
                        },
                    },
                ],
            },
        }],
    };
    let root_manifest = ws.store.put_manifest(&manifest)?;
    let snap = SnapRecord {
        version: 2,
        id: compute_snap_id(&root_manifest, &[], None),
        created_at: "2026-07-25T00:00:00Z".into(),
        root_manifest,
        parents: Vec::new(),
        derived_from_bundle: None,
        message: None,
        trigger: "explicit".into(),
        stats: SnapStats::default(),
    };
    ws.store.put_snap(&snap)?;

    let err = export_lineage(&ws.store, root, "converge/lane/local", &snap.id).unwrap_err();
    assert!(format!("{err:#}").contains("resolve before export"));
    Ok(())
}

/// Audit G2: a crash between fast-import and the map save must not
/// yield duplicate commits on re-export. Simulated by deleting the
/// git-map after a successful export — the deterministic fast-import
/// recreates identical shas and the branch history stays single.
#[test]
fn lost_map_reexport_produces_no_duplicate_commits() -> Result<()> {
    if !git_available() {
        eprintln!("git not available; skipping");
        return Ok(());
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    git_out(root, &["init", "--quiet"])?;
    let ws = Workspace::init(root, false)?;

    std::fs::write(root.join("a.txt"), "one")?;
    ws.create_snap(Some("first".into()))?;
    std::fs::write(root.join("a.txt"), "two")?;
    let s2 = ws.create_snap(Some("second".into()))?;

    export_lineage(&ws.store, root, "converge/lane/local", &s2.id)?;
    let head_before = git_out(root, &["rev-parse", "converge/lane/local"])?;

    // Crash simulation: map lost after commits landed.
    std::fs::remove_file(ws.store.root_dir().join("git-map.json"))?;

    let report = export_lineage(&ws.store, root, "converge/lane/local", &s2.id)?;
    assert_eq!(report.exported_commits, 2, "full re-export after map loss");

    let head_after = git_out(root, &["rev-parse", "converge/lane/local"])?;
    assert_eq!(head_before, head_after, "deterministic shas");
    let count = git_out(root, &["rev-list", "--count", "converge/lane/local"])?;
    assert_eq!(count.trim(), "2", "no duplicate history");
    // Temp export ref cleaned up.
    assert!(
        git_out(root, &["rev-parse", "refs/converge/export-tmp"]).is_err(),
        "temp ref deleted"
    );
    Ok(())
}
