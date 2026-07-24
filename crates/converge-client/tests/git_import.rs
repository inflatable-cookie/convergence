//! Doc 18 §3 import: seed, first-parent history, ignore translation,
//! import->export no-duplication. Requires `git` on PATH; skips without.

use std::process::Command;

use anyhow::Result;

use converge_client::git_export::export_lineage;
use converge_client::git_import::{ImportDepth, import};
use converge_client::workspace::Workspace;

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

fn seeded_git_repo() -> Result<tempfile::TempDir> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    git(root, &["init", "--quiet"])?;
    std::fs::write(root.join("a.txt"), "one")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "first commit"])?;
    std::fs::write(root.join("a.txt"), "two")?;
    std::fs::write(root.join("b.txt"), "beta")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "second commit"])?;
    std::fs::write(root.join("c.txt"), "gamma")?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "third commit"])?;
    Ok(tmp)
}

#[test]
fn seed_import_captures_tree_with_trailer() -> Result<()> {
    if !git_available() {
        return Ok(());
    }
    let tmp = seeded_git_repo()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let report = import(&ws, ImportDepth::Seed)?;
    assert_eq!(report.imported_snaps, 1);

    let snap = ws.store.get_snap(&report.head_snap_id)?;
    assert!(
        snap.message
            .as_deref()
            .unwrap()
            .contains("Converge-Imported-Commit: "),
        "trailer present"
    );
    assert_eq!(snap.stats.files, 3, "current tree captured");
    Ok(())
}

#[test]
fn history_import_wires_lineage_and_export_does_not_duplicate() -> Result<()> {
    if !git_available() {
        return Ok(());
    }
    let tmp = seeded_git_repo()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let report = import(&ws, ImportDepth::All)?;
    assert_eq!(report.imported_snaps, 3);

    // Lineage wired oldest -> newest; messages preserved.
    let snaps = ws.list_snaps()?;
    assert_eq!(snaps.len(), 3);
    assert!(
        snaps[0]
            .message
            .as_deref()
            .unwrap()
            .contains("third commit")
    );
    assert_eq!(snaps[0].parents, vec![snaps[1].id.clone()]);
    assert_eq!(snaps[1].parents, vec![snaps[2].id.clone()]);
    assert!(snaps[2].parents.is_empty());

    // Historical trees are correct: restore the oldest import.
    ws.restore_snap(&snaps[2].id, true)?;
    assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt"))?, "one");
    assert!(!tmp.path().join("b.txt").exists());
    ws.restore_snap(&snaps[0].id, true)?;

    // Import -> export round trip: everything already mapped, nothing
    // re-exported (doc 18: the map carries correspondence both ways).
    let export = export_lineage(&ws.store, tmp.path(), "converge/lane/local", &snaps[0].id)?;
    assert_eq!(export.exported_commits, 0, "no duplicate history");
    assert_eq!(export.skipped_existing, 3);
    Ok(())
}

#[test]
fn gitignore_translates_and_capture_honors_it() -> Result<()> {
    if !git_available() {
        return Ok(());
    }
    let tmp = seeded_git_repo()?;
    let root = tmp.path();
    std::fs::write(
        root.join(".gitignore"),
        "# comment\nbuild/\n!keep.txt\ntarget\n",
    )?;
    git(root, &["add", "."])?;
    git(root, &["commit", "--quiet", "-m", "ignore file"])?;
    std::fs::create_dir(root.join("build"))?;
    std::fs::write(root.join("build/artifact.bin"), "junk")?;

    let ws = Workspace::init(root, false)?;
    let report = import(&ws, ImportDepth::Seed)?;
    assert!(report.translated_ignores);
    let generated = std::fs::read_to_string(root.join(".convergeignore"))?;
    assert!(generated.contains("build/"));
    assert!(!generated.contains("!keep.txt"), "negations dropped");

    // Capture after translation excludes the ignored dir.
    std::fs::write(root.join("new.txt"), "x")?;
    let snap = ws.create_snap(None)?;
    let manifest = ws.store.get_manifest(&snap.root_manifest)?;
    assert!(
        !manifest.entries.iter().any(|e| e.name == "build"),
        "root ignore honored"
    );
    assert!(manifest.entries.iter().any(|e| e.name == "new.txt"));
    Ok(())
}
