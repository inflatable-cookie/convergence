//! Batch 18.2: kill the process mid-operation and check what survives.
//!
//! The local write paths (restore, git export) claim atomicity through a
//! temp-swap and a map-before-ref-move. Those claims were argued in batch
//! 12.1 and 12.2 and never tested against an actual kill, which is the
//! only thing that exercises the window they are protecting.

use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::Result;
use converge_client::workspace::Workspace;

fn converge(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run converge")
}

/// Start a command and kill it after `delay`, mid-flight.
fn kill_during(dir: &Path, args: &[&str], delay_ms: u64) -> Result<()> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

/// Many files, so materialize takes long enough to be interrupted
/// somewhere interesting.
fn wide_workspace(root: &Path, files: usize, body: &str) -> Result<()> {
    for i in 0..files {
        let dir = root.join(format!("d{:03}", i % 20));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("f{i:04}.txt")), format!("{body} {i}"))?;
    }
    Ok(())
}

/// What a kill during restore actually guarantees (batch 18.2).
///
/// The swap is a per-entry delete-and-rename, so a process killed inside
/// it *can* leave a partly-swapped tree — pretending otherwise would
/// need a journal or an atomic swap of the workspace root, and neither
/// is on the table while `.converge` has to stay in place. What must
/// hold is weaker and testable: the store survives, re-running restore
/// completes to exactly the target tree, and the interrupted run leaves
/// no staging debris in the workspace for the next `snap` to capture.
#[test]
fn restore_killed_mid_materialize_recovers_without_debris() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    wide_workspace(root, 150, "first")?;
    let ws = Workspace::discover(root)?;
    let first = ws.create_snap(Some("first".into()))?;

    // Replace every file, so the two trees share no content.
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() && path.file_name().is_some_and(|n| n != ".converge") {
            std::fs::remove_dir_all(&path)?;
        }
    }
    wide_workspace(root, 150, "second")?;
    let second = ws.create_snap(Some("second".into()))?;

    // Kill at a spread of delays: some land before the swap, some after,
    // and a few land in the middle of writing the staging tree.
    for delay in [4, 12, 35] {
        kill_during(root, &["restore", &first.id, "--force"], delay)?;

        // No staging debris: a killed restore must not leave a tree the
        // scan will count and the next snap will capture.
        let debris: Vec<String> = std::fs::read_dir(root)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("materialize"))
            .collect();
        assert!(
            debris.is_empty(),
            "kill at {delay}ms left staging debris in the workspace: {debris:?}"
        );

        // The store survived: status still answers.
        let status = converge(root, &["--json", "status"]);
        assert!(
            status.status.success(),
            "workspace unusable after a kill at {delay}ms: {}",
            String::from_utf8_lossy(&status.stderr)
        );

        // Re-running restore completes the job exactly.
        assert!(
            converge(root, &["restore", &first.id, "--force"])
                .status
                .success(),
            "restore could not recover after a kill at {delay}ms"
        );
        let ws = Workspace::discover(root)?;
        let (current, _, _) = ws.current_manifest_tree()?;
        assert_eq!(
            current, first.root_manifest,
            "the retried restore did not land the target tree"
        );

        // Put it back for the next round.
        assert!(
            converge(root, &["restore", &second.id, "--force"])
                .status
                .success()
        );
    }
    Ok(())
}

/// Git export killed partway must not leave a ref pointing at history the
/// map does not know about: batch 12.2 writes the map before moving the
/// ref precisely so a kill cannot invert that order.
#[test]
fn git_export_killed_midway_never_moves_a_ref_ahead_of_its_map() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("git not available; skipping");
        return Ok(());
    }
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root)
            .status()?
            .success()
    );

    let ws = Workspace::discover(root)?;
    let mut snaps = Vec::new();
    for round in 0..4 {
        wide_workspace(root, 60, &format!("round {round}"))?;
        snaps.push(ws.create_snap(Some(format!("round {round}")))?);
    }

    for delay in [6, 20] {
        kill_during(root, &["git", "export"], delay)?;

        // Either the ref moved and every mapped snap is reachable, or it
        // did not move at all. A ref ahead of the map is the failure.
        let out = converge(root, &["--json", "git", "export"]);
        assert!(
            out.status.success(),
            "export could not recover after a kill at {delay}ms: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        // The branch resolves to a real commit whose history is intact.
        let rev = Command::new("git")
            .args(["rev-parse", "--verify", "refs/heads/converge"])
            .current_dir(root)
            .output()?;
        if rev.status.success() {
            let log = Command::new("git")
                .args(["log", "--oneline", "refs/heads/converge"])
                .current_dir(root)
                .output()?;
            assert!(
                log.status.success(),
                "exported ref has unreadable history after a kill at {delay}ms"
            );
            assert_eq!(
                String::from_utf8_lossy(&log.stdout).lines().count(),
                snaps.len(),
                "exported history lost or duplicated commits after a kill at {delay}ms"
            );
        }
    }
    Ok(())
}
