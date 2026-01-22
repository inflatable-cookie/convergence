mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

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

#[derive(Debug, serde::Deserialize)]
struct Publication {
    id: String,
}

#[derive(Debug, serde::Deserialize)]
struct Bundle {
    promotable: bool,
    reasons: Vec<String>,
}

#[test]
fn bundle_is_not_promotable_when_superpositions_present() -> Result<()> {
    let server = common::spawn_server()?;
    let base_url = server.base_url.clone();
    let token = server.token.clone();

    let ws1 = tempfile::tempdir().context("create ws1")?;
    let ws2 = tempfile::tempdir().context("create ws2")?;

    // Configure both workspaces.
    for ws in [&ws1, &ws2] {
        run_converge(ws.path(), &["init"])?;
        run_converge(
            ws.path(),
            &[
                "remote",
                "set",
                "--url",
                &base_url,
                "--token",
                &token,
                "--repo",
                "test",
                "--scope",
                "main",
                "--gate",
                "dev-intake",
            ],
        )?;
    }

    run_converge(ws1.path(), &["remote", "create-repo"])?;

    // Publish two different versions of the same file.
    fs::write(ws1.path().join("a.txt"), b"one\n").context("write a.txt ws1")?;
    let snap1 = run_converge(ws1.path(), &["snap", "-m", "one"])?;
    let pub1: Publication = serde_json::from_str(&run_converge(
        ws1.path(),
        &["publish", "--snap-id", &snap1, "--json"],
    )?)
    .context("parse pub1")?;

    fs::write(ws2.path().join("a.txt"), b"two\n").context("write a.txt ws2")?;
    let snap2 = run_converge(ws2.path(), &["snap", "-m", "two"])?;
    let pub2: Publication = serde_json::from_str(&run_converge(
        ws2.path(),
        &["publish", "--snap-id", &snap2, "--json"],
    )?)
    .context("parse pub2")?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{}/repos/test/bundles", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pub1.id, pub2.id]
        }))
        .send()
        .context("create bundle")?
        .error_for_status()
        .context("create bundle status")?;

    let bundle: Bundle = resp.json().context("parse bundle")?;
    assert!(!bundle.promotable);
    assert!(bundle.reasons.iter().any(|r| r == "superpositions_present"));
    Ok(())
}
