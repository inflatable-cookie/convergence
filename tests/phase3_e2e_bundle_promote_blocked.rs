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

fn run_converge_expect_failure(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(cwd)
        .args(args)
        .output()
        .with_context(|| format!("run converge {:?} in {}", args, cwd.display()))?;

    if out.status.success() {
        anyhow::bail!("expected failure for converge {:?}", args);
    }

    Ok(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[derive(Debug, serde::Deserialize)]
struct Bundle {
    id: String,
    promotable: bool,
    reasons: Vec<String>,
}

fn setup_workspace(ws: &Path, base_url: &str, token: &str) -> Result<()> {
    run_converge(ws, &["init"])?;
    run_converge(
        ws,
        &[
            "remote",
            "set",
            "--url",
            base_url,
            "--token",
            token,
            "--repo",
            "test",
            "--scope",
            "main",
            "--gate",
            "dev-intake",
        ],
    )?;
    Ok(())
}

#[test]
fn phase3_e2e_bundle_with_conflict_blocks_promotion() -> Result<()> {
    let server = common::spawn_server()?;
    let base_url = server.base_url.clone();
    let token = server.token.clone();

    let ws1 = tempfile::tempdir().context("create ws1")?;
    let ws2 = tempfile::tempdir().context("create ws2")?;

    setup_workspace(ws1.path(), &base_url, &token)?;
    setup_workspace(ws2.path(), &base_url, &token)?;

    run_converge(ws1.path(), &["remote", "create-repo"])?;

    // Configure a simple 2-gate graph: dev-intake -> team
    let client = reqwest::blocking::Client::new();
    client
        .put(format!("{}/repos/test/gate-graph", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({
            "version": 1,
            "terminal_gate": "team",
            "gates": [
                {"id": "dev-intake", "name": "Dev Intake", "upstream": [], "allow_superpositions": false},
                {"id": "team", "name": "Team", "upstream": ["dev-intake"], "allow_superpositions": false}
            ]
        }))
        .send()
        .context("put gate graph")?
        .error_for_status()
        .context("put gate graph status")?;

    // Publish two conflicting snaps to dev-intake.
    fs::write(ws1.path().join("a.txt"), b"one\n").context("write a.txt ws1")?;
    let snap1 = run_converge(ws1.path(), &["snap", "-m", "one"])?;
    run_converge(ws1.path(), &["publish", "--snap-id", &snap1])?;

    fs::write(ws2.path().join("a.txt"), b"two\n").context("write a.txt ws2")?;
    let snap2 = run_converge(ws2.path(), &["snap", "-m", "two"])?;
    run_converge(ws2.path(), &["publish", "--snap-id", &snap2])?;

    // Bundle all publications for (main, dev-intake).
    let bundle_json = run_converge(ws1.path(), &["bundle", "--json"])?;
    let bundle: Bundle = serde_json::from_str(&bundle_json).context("parse bundle")?;
    assert!(!bundle.promotable);
    assert!(bundle.reasons.iter().any(|r| r == "superpositions_present"));

    // Promotion should be blocked.
    let err = run_converge_expect_failure(
        ws1.path(),
        &["promote", "--bundle-id", &bundle.id, "--to-gate", "team"],
    )?;
    assert!(
        err.contains("409") || err.to_lowercase().contains("conflict"),
        "expected conflict, got: {}",
        err
    );

    Ok(())
}

#[test]
fn phase3_e2e_clean_bundle_can_be_promoted() -> Result<()> {
    let server = common::spawn_server()?;
    let base_url = server.base_url.clone();
    let token = server.token.clone();

    let ws = tempfile::tempdir().context("create ws")?;
    setup_workspace(ws.path(), &base_url, &token)?;
    run_converge(ws.path(), &["remote", "create-repo"])?;

    // Configure a simple 2-gate graph: dev-intake -> team
    let client = reqwest::blocking::Client::new();
    client
        .put(format!("{}/repos/test/gate-graph", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({
            "version": 1,
            "terminal_gate": "team",
            "gates": [
                {"id": "dev-intake", "name": "Dev Intake", "upstream": [], "allow_superpositions": false},
                {"id": "team", "name": "Team", "upstream": ["dev-intake"], "allow_superpositions": false}
            ]
        }))
        .send()
        .context("put gate graph")?
        .error_for_status()
        .context("put gate graph status")?;

    // Publish a clean snap.
    fs::write(ws.path().join("a.txt"), b"ok\n").context("write a.txt")?;
    let snap = run_converge(ws.path(), &["snap", "-m", "ok"])?;
    run_converge(ws.path(), &["publish", "--snap-id", &snap])?;

    // Bundle should be promotable.
    let bundle_json = run_converge(ws.path(), &["bundle", "--json"])?;
    let bundle: Bundle = serde_json::from_str(&bundle_json).context("parse bundle")?;
    assert!(bundle.promotable);
    assert!(bundle.reasons.is_empty());

    // Promotion should succeed.
    run_converge(
        ws.path(),
        &["promote", "--bundle-id", &bundle.id, "--to-gate", "team"],
    )?;

    // Verify promotion state updated.
    let state: serde_json::Value = client
        .get(format!(
            "{}/repos/test/promotion-state?scope=main",
            base_url
        ))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .send()
        .context("get promotion state")?
        .error_for_status()
        .context("get promotion state status")?
        .json()
        .context("parse promotion state")?;
    assert_eq!(
        state.get("team"),
        Some(&serde_json::Value::String(bundle.id))
    );

    Ok(())
}
