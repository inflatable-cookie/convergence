use anyhow::{Context, Result};

#[allow(dead_code)]
mod common;

#[test]
fn server_bootstrap_creates_first_admin_once() -> Result<()> {
    let data_dir = tempfile::tempdir().context("create server tempdir")?;
    let addr_file = data_dir.path().join("addr.txt");

    let bootstrap_token = "bootstrap-secret";

    let (mut child, base_url) = common::spawn_server_process(
        data_dir.path(),
        &addr_file,
        &["--bootstrap-token", bootstrap_token],
    )?;
    common::wait_for_healthz(&base_url)?;

    let client = reqwest::blocking::Client::new();

    // Bootstrap first admin.
    let resp = client
        .post(format!("{}/bootstrap", base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", bootstrap_token),
        )
        .json(&serde_json::json!({"handle": "admin"}))
        .send()
        .context("POST /bootstrap")?;
    anyhow::ensure!(resp.status().is_success(), "bootstrap failed");
    let out: serde_json::Value = resp.json().context("parse bootstrap response")?;
    let token = out
        .get("token")
        .and_then(|t| t.get("token"))
        .and_then(|t| t.as_str())
        .context("missing bootstrap token")?
        .to_string();

    // Verify whoami works with minted token.
    let resp = client
        .get(format!("{}/whoami", base_url))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .context("GET /whoami")?;
    anyhow::ensure!(resp.status().is_success(), "whoami failed");
    let who: serde_json::Value = resp.json().context("parse whoami")?;
    anyhow::ensure!(who.get("user").and_then(|v| v.as_str()) == Some("admin"));
    anyhow::ensure!(who.get("admin").and_then(|v| v.as_bool()) == Some(true));

    // Second bootstrap should be rejected.
    let resp = client
        .post(format!("{}/bootstrap", base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", bootstrap_token),
        )
        .json(&serde_json::json!({"handle": "admin2"}))
        .send()
        .context("POST /bootstrap (second)")?;
    anyhow::ensure!(resp.status() == reqwest::StatusCode::CONFLICT);

    let _ = child.kill();
    let _ = child.wait();

    Ok(())
}
