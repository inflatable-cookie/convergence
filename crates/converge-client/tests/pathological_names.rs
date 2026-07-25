//! Batch 18.3: filenames users actually create, and a few they should
//! not be able to break us with.
//!
//! The audit called this out by name: no unicode or pathological
//! filename coverage anywhere, which is how a fast-import path bug ships.
//! Capture → snap → git export is the full local path a name travels.

use std::process::Command;

use anyhow::Result;
use converge_client::workspace::Workspace;

/// Names that are legal on a POSIX filesystem and awkward everywhere
/// else. Each has bitten a real version-control system.
fn hostile_names() -> Vec<String> {
    vec![
        "plain.txt".to_string(),
        "with space.txt".to_string(),
        "-leading-dash.txt".to_string(),
        "trailing.space .txt".to_string(),
        "quote\"inside.txt".to_string(),
        "back\\slash.txt".to_string(),
        "new\nline.txt".to_string(),
        "tab\there.txt".to_string(),
        "émoji-🎛️-mix.txt".to_string(),
        // Same grapheme, different normalisation: NFC vs NFD. A store
        // that normalises would collapse these into one path and lose a
        // file; one that does not must keep both.
        "café.txt".to_string(),
        "cafe\u{301}.txt".to_string(),
        "ünïcödé/nested.txt".to_string(),
        "very.long.name.with.many.dots.and-dashes_and_underscores.txt".to_string(),
        "#hash.txt".to_string(),
        "semi;colon.txt".to_string(),
        "star*not-a-glob.txt".to_string(),
    ]
}

/// Write every name the platform accepts, then report what the
/// filesystem actually ended up holding.
///
/// The two are not the same thing, and the difference is the point:
/// macOS normalises unicode, so `café.txt` (NFC) and `cafe\u{301}.txt`
/// (NFD) are one file there and two on Linux. Reading the tree back
/// keeps this a test of Convergence rather than of the filesystem.
fn write_all(root: &std::path::Path) -> Result<Vec<String>> {
    for (i, name) in hostile_names().iter().enumerate() {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::write(&path, format!("content {i}"));
    }
    let mut present = Vec::new();
    collect_files(root, root, &mut present)?;
    present.sort();
    Ok(present)
}

fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".converge" || name == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            out.push(
                path.strip_prefix(root)?
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            );
        }
    }
    Ok(())
}

/// Capture, restore, and diff must round-trip every name the filesystem
/// accepted — byte for byte, no normalisation, no loss.
#[test]
fn hostile_names_round_trip_through_capture_and_restore() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;
    let written = write_all(root)?;
    assert!(written.len() > 8, "the platform rejected too much to test");

    let snap = ws.create_snap(Some("hostile".into()))?;
    assert_eq!(
        snap.stats.files as usize,
        written.len(),
        "every accepted name was captured"
    );

    // Wipe and restore: content and names must come back identical.
    for name in &written {
        let path = root.join(name);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    ws.restore_snap(&snap.id, true)?;
    let mut restored = Vec::new();
    collect_files(root, root, &mut restored)?;
    restored.sort();
    assert_eq!(restored, written, "restore lost or renamed a path");
    for name in &written {
        assert!(
            std::fs::read_to_string(root.join(name))?.starts_with("content "),
            "{name:?} came back with the wrong content"
        );
    }

    // Recapture is idempotent: the same tree hashes to the same snap.
    let again = ws.create_snap(None)?;
    assert_eq!(again.id, snap.id, "restored tree is not identical");

    // Where the filesystem kept both normalisation forms, Convergence
    // must keep both too: normalising in the store would silently merge
    // two files a user can see side by side.
    let nfc = written.iter().any(|n| n == "café.txt");
    let nfd = written.iter().any(|n| n == "cafe\u{301}.txt");
    if nfc && nfd {
        let manifest = ws.store.get_manifest(&snap.root_manifest)?;
        let names: Vec<&str> = manifest.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"café.txt") && names.contains(&"cafe\u{301}.txt"),
            "normalisation collapsed two distinct paths: {names:?}"
        );
    }
    Ok(())
}

/// The same names through `git export`. A path containing a newline or a
/// quote has to be C-quoted in a fast-import stream, or the stream is
/// silently reinterpreted — a filename becomes a command.
#[test]
fn hostile_names_survive_git_export() -> Result<()> {
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("git not available; skipping");
        return Ok(());
    }
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root)
            .status()?
            .success()
    );

    let written = write_all(root)?;
    let snap = ws.create_snap(Some("hostile".into()))?;
    let report =
        converge_client::git_export::export_lineage(&ws.store, root, "converge", &snap.id)?;
    assert!(report.exported_commits > 0, "nothing exported");

    // Git's own view of the exported tree must list exactly what we
    // captured. `-z` avoids git's display quoting so the comparison is
    // over real bytes.
    let listing = Command::new("git")
        .args(["ls-tree", "-r", "-z", "--name-only", "refs/heads/converge"])
        .current_dir(root)
        .output()?;
    assert!(
        listing.status.success(),
        "ls-tree failed: {}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let mut in_git: Vec<String> = String::from_utf8_lossy(&listing.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    in_git.sort();
    assert_eq!(
        in_git, written,
        "exported tree does not match the captured one"
    );
    Ok(())
}
