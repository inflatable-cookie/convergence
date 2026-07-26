//! Batch 22.2: an incompatible store is refused with an explanation
//! rather than misread.
//!
//! The failure being prevented is not a crash. A crash would be fine.
//! It is a newer binary silently misreading an older store — a field
//! that gained a meaning, an id whose domain tag changed (batch 18.3
//! moved `converge-snap-v3` to `v4`). Those corrupt quietly.

use std::path::Path;
use std::process::{Command, Output};

use anyhow::Result;

fn converge(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", dir.join("home"))
        .args(args)
        .output()
        .expect("run converge")
}

fn stamp(dir: &Path) -> std::path::PathBuf {
    dir.join(".converge").join("format")
}

#[test]
fn a_fresh_workspace_is_stamped_with_the_current_format() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    assert_eq!(
        std::fs::read_to_string(stamp(dir.path()))?.trim(),
        "converge-workspace-1"
    );
    Ok(())
}

/// A store written before the stamp existed reads as version 1, and
/// opening it does not write one — `doctor` opens a workspace and is
/// tested to change nothing (batch 22.1).
#[test]
fn an_unstamped_workspace_still_works_and_stays_unstamped() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    std::fs::remove_file(stamp(dir.path()))?;

    let out = converge(dir.path(), &["status"]);
    assert!(
        out.status.success(),
        "an unstamped store predates the stamp; it is version 1, not broken: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !stamp(dir.path()).exists(),
        "opening a store wrote to it; that makes `doctor` a mutation"
    );
    Ok(())
}

/// Every path in, not just the one the test author thought of.
#[test]
fn an_incompatible_workspace_is_refused_by_every_verb() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    std::fs::write(stamp(dir.path()), "converge-workspace-99\n")?;

    for verb in [
        "status", "history", "changes", "snap", "publish", "remote", "inbox",
    ] {
        let out = converge(dir.path(), &[verb]);
        assert!(!out.status.success(), "`{verb}` ran against format 99");
        let message = String::from_utf8_lossy(&out.stderr);
        assert!(
            message.contains("format 99"),
            "`{verb}` refused without naming the version: {message}"
        );
        assert!(
            message.contains("Nothing has been read or written"),
            "`{verb}` refused without saying it was safe: {message}"
        );
    }
    Ok(())
}

/// The one that matters most, found by driving it: every verb refused a
/// format-99 workspace, and then `init --force` cheerfully reset it to
/// format 1 — destroying exactly the history the refusal was protecting.
#[test]
fn force_init_will_not_destroy_a_store_it_cannot_read() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    std::fs::write(stamp(dir.path()), "converge-workspace-99\n")?;

    let out = converge(dir.path(), &["init", "--force"]);
    let message = String::from_utf8_lossy(&out.stderr);
    assert!(
        message.contains("will not re-initialise"),
        "`--force` means 're-init over my own store', not 'destroy one I \
         cannot read': {message}"
    );
    assert!(
        message.contains("remove") && message.contains(".converge"),
        "and it should say what to do if they really mean it: {message}"
    );
    assert_eq!(
        std::fs::read_to_string(stamp(dir.path()))?.trim(),
        "converge-workspace-99",
        "the store was overwritten anyway"
    );
    Ok(())
}

/// `doctor` must not send someone to the command that would destroy the
/// thing (batch 22.1's own advice was wrong here until this batch).
#[test]
fn doctor_does_not_recommend_init_for_a_format_mismatch() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    std::fs::write(stamp(dir.path()), "converge-workspace-99\n")?;

    let out = converge(dir.path(), &["--json", "doctor"]);
    let report: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim())?;
    let workspace = report["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == "workspace")
        .expect("workspace check");
    let fix = workspace["fix"].as_str().unwrap_or("");
    assert!(
        !fix.contains("converge init"),
        "recommending init here points at the one command that destroys it: {fix}"
    );
    assert!(fix.contains("do NOT"), "and it should say so: {fix}");
    Ok(())
}

/// A healthy workspace reports its format, so "what version is this"
/// has an answer that is not `cat`.
#[test]
fn doctor_reports_the_store_format() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(converge(dir.path(), &["init"]).status.success());
    let out = converge(dir.path(), &["--json", "doctor"]);
    let report: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim())?;
    let format = report["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == "store format")
        .expect("store format check");
    assert_eq!(format["ok"], true);
    assert_eq!(format["detail"], "version 1");
    Ok(())
}
