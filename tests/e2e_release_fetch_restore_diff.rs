use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

mod common;

fn run_converge(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(cwd)
        .args(args)
        .output()
        .with_context(|| format!("run converge {:?} in {}", args, cwd.display()))?;

    if !out.status.success() {
        anyhow::bail!(
            "converge {:?} failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn write_fixture(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("sub")).context("create sub dir")?;
    fs::write(dir.join("a.txt"), b"hello\n").context("write a.txt")?;
    fs::write(dir.join("sub/b.txt"), b"world\n").context("write b.txt")?;
    Ok(())
}

#[test]
fn e2e_release_fetch_restore_and_diff_against_original_snap() -> Result<()> {
    let server = common::spawn_server()?;

    let ws1 = tempfile::tempdir().context("create ws1")?;
    let ws2 = tempfile::tempdir().context("create ws2")?;
    let out = tempfile::tempdir().context("create out")?;

    // Workspace 1: publish -> bundle -> release.
    run_converge(ws1.path(), &["init"])?;
    run_converge(
        ws1.path(),
        &[
            "login",
            "--url",
            &server.base_url,
            "--token",
            &server.token,
            "--repo",
            "test",
            "--scope",
            "main",
            "--gate",
            "dev-intake",
        ],
    )?;
    run_converge(ws1.path(), &["remote", "create-repo"])?;

    write_fixture(ws1.path())?;

    let snap_id = run_converge(ws1.path(), &["snap", "-m", "release-e2e"])?
        .trim()
        .to_string();
    run_converge(ws1.path(), &["publish", "--snap-id", &snap_id])?;
    let bundle_id = run_converge(ws1.path(), &["bundle"])?.trim().to_string();
    run_converge(
        ws1.path(),
        &[
            "release",
            "create",
            "--channel",
            "stable",
            "--bundle-id",
            &bundle_id,
        ],
    )?;

    // Workspace 2: fetch release and materialize into out_dir.
    run_converge(ws2.path(), &["init"])?;
    run_converge(
        ws2.path(),
        &[
            "login",
            "--url",
            &server.base_url,
            "--token",
            &server.token,
            "--repo",
            "test",
        ],
    )?;
    run_converge(
        ws2.path(),
        &[
            "fetch",
            "--release",
            "stable",
            "--restore",
            "--into",
            out.path().to_str().unwrap(),
            "--force",
        ],
    )?;

    // Turn out_dir into a workspace, fetch the original snap, and diff.
    run_converge(out.path(), &["init"])?;
    run_converge(
        out.path(),
        &[
            "login",
            "--url",
            &server.base_url,
            "--token",
            &server.token,
            "--repo",
            "test",
        ],
    )?;
    run_converge(out.path(), &["fetch", "--snap-id", &snap_id])?;
    let snap2 = run_converge(out.path(), &["snap", "-m", "restored"])?
        .trim()
        .to_string();

    let diff = run_converge(
        out.path(),
        &["diff", "--from", &snap_id, "--to", &snap2, "--json"],
    )?;
    let v: serde_json::Value = serde_json::from_str(&diff).context("parse diff json")?;
    let arr = v.as_array().context("diff json not array")?;
    assert!(arr.is_empty(), "expected no diff, got: {}", diff);

    Ok(())
}
