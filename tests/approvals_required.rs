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
struct Bundle {
    id: String,
    promotable: bool,
    reasons: Vec<String>,
    approvals: Vec<String>,
}

#[test]
fn approvals_make_bundle_promotable() -> Result<()> {
    let server = common::spawn_server()?;
    let base_url = server.base_url.clone();
    let token = server.token.clone();

    let ws = tempfile::tempdir().context("create ws")?;
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
    run_converge(ws.path(), &["remote", "create-repo"])?;

    // Gate graph: dev-intake requires 1 approval.
    let client = reqwest::blocking::Client::new();
    client
        .put(format!("{}/repos/test/gate-graph", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({
            "version": 1,
            "terminal_gate": "dev-intake",
            "gates": [
                {"id": "dev-intake", "name": "Dev Intake", "upstream": [], "allow_superpositions": false, "required_approvals": 1}
            ]
        }))
        .send()
        .context("put gate graph")?
        .error_for_status()
        .context("put gate graph status")?;

    fs::write(ws.path().join("a.txt"), b"ok\n").context("write a.txt")?;
    let snap = run_converge(ws.path(), &["snap", "-m", "ok"])?;
    run_converge(ws.path(), &["publish", "--snap-id", &snap])?;

    let bundle_json = run_converge(ws.path(), &["bundle", "--json"])?;
    let bundle: Bundle = serde_json::from_str(&bundle_json).context("parse bundle")?;
    assert!(!bundle.promotable);
    assert!(bundle.reasons.iter().any(|r| r == "approvals_missing"));

    // Approve and verify it becomes promotable.
    let resp = client
        .post(format!(
            "{}/repos/test/bundles/{}/approve",
            base_url, bundle.id
        ))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .send()
        .context("approve bundle")?
        .error_for_status()
        .context("approve bundle status")?;
    let updated: Bundle = resp.json().context("parse approved bundle")?;
    assert!(updated.promotable);
    assert!(updated.reasons.is_empty());
    assert_eq!(updated.approvals.len(), 1);

    Ok(())
}
