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
    id: String,
    promotable: bool,
}

#[derive(Debug, serde::Deserialize)]
struct Promotion {
    to_gate: String,
}

fn configure_gate_graph(base_url: &str, token: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    client
        .put(format!("{}/repos/test/gate-graph", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(token))
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
    Ok(())
}

#[test]
fn promotable_bundle_can_be_promoted() -> Result<()> {
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

    configure_gate_graph(&base_url, &token)?;

    fs::write(ws.path().join("a.txt"), b"ok\n").context("write a.txt")?;
    let snap = run_converge(ws.path(), &["snap", "-m", "ok"])?;
    let pubrec: Publication = serde_json::from_str(&run_converge(
        ws.path(),
        &["publish", "--snap-id", &snap, "--json"],
    )?)
    .context("parse publication")?;

    let client = reqwest::blocking::Client::new();
    let bundle_resp = client
        .post(format!("{}/repos/test/bundles", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pubrec.id]
        }))
        .send()
        .context("create bundle")?
        .error_for_status()
        .context("create bundle status")?;

    let bundle: Bundle = bundle_resp.json().context("parse bundle")?;
    assert!(bundle.promotable);

    let prom_resp = client
        .post(format!("{}/repos/test/promotions", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({"bundle_id": bundle.id, "to_gate": "team"}))
        .send()
        .context("promote")?
        .error_for_status()
        .context("promote status")?;

    let prom: Promotion = prom_resp.json().context("parse promotion")?;
    assert_eq!(prom.to_gate, "team");

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

    assert!(state.get("team").is_some());
    Ok(())
}

#[test]
fn non_promotable_bundle_cannot_be_promoted() -> Result<()> {
    let server = common::spawn_server()?;
    let base_url = server.base_url.clone();
    let token = server.token.clone();

    let ws1 = tempfile::tempdir().context("create ws1")?;
    let ws2 = tempfile::tempdir().context("create ws2")?;

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
    configure_gate_graph(&base_url, &token)?;

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
    let bundle_resp = client
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

    let bundle: Bundle = bundle_resp.json().context("parse bundle")?;
    assert!(!bundle.promotable);

    let resp = client
        .post(format!("{}/repos/test/promotions", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .json(&serde_json::json!({"bundle_id": bundle.id, "to_gate": "team"}))
        .send()
        .context("promote")?;

    assert_eq!(resp.status(), reqwest::StatusCode::CONFLICT);
    Ok(())
}
