use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

#[allow(dead_code)]
mod common;

fn read_addr_file(addr_file: &std::path::Path) -> Result<String> {
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(5) {
            anyhow::bail!("addr file not written at {}", addr_file.display());
        }

        if let Ok(s) = std::fs::read_to_string(addr_file) {
            let s = s.trim();
            if !s.is_empty() {
                return Ok(format!("http://{}", s));
            }
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn spawn_server(
    data_dir: &std::path::Path,
    addr_file: &std::path::Path,
) -> Result<(Child, String)> {
    let token = "dev";

    let child = Command::new(env!("CARGO_BIN_EXE_converge-server"))
        .args([
            "--addr",
            "127.0.0.1:0",
            "--addr-file",
            addr_file.to_str().unwrap(),
            "--data-dir",
            data_dir.to_str().unwrap(),
            "--dev-user",
            "dev",
            "--dev-token",
            token,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn converge-server")?;

    let base_url = read_addr_file(addr_file)?;
    common::wait_for_healthz(&base_url)?;
    Ok((child, base_url))
}

#[test]
fn server_persists_repo_across_restart() -> Result<()> {
    let data_dir = tempfile::tempdir().context("create temp data dir")?;
    let data_dir_path = data_dir.path();

    let addr1 = data_dir_path.join("addr1.txt");
    let (mut child1, base_url1) = spawn_server(data_dir_path, &addr1)?;

    let client = reqwest::blocking::Client::new();
    let auth = common::auth_header("dev");

    // Create repo.
    client
        .post(format!("{}/repos", base_url1))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({"id": "test"}))
        .send()
        .context("create repo")?
        .error_for_status()
        .context("create repo status")?;

    // Upload an empty manifest.
    let manifest = converge::model::Manifest {
        version: 1,
        entries: vec![],
    };
    let manifest_bytes = serde_json::to_vec(&manifest).context("serialize manifest")?;
    let manifest_id = blake3::hash(&manifest_bytes).to_hex().to_string();

    client
        .put(format!(
            "{}/repos/test/objects/manifests/{}",
            base_url1, manifest_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(manifest_bytes)
        .send()
        .context("put manifest")?
        .error_for_status()
        .context("put manifest status")?;

    // Upload snap.
    let created_at = "2026-01-22T00:00:00Z";
    let root_manifest = converge::model::ObjectId(manifest_id.clone());
    let snap_id = converge::model::compute_snap_id(created_at, &root_manifest, None);
    let snap = converge::model::SnapRecord {
        version: 1,
        id: snap_id.clone(),
        created_at: created_at.to_string(),
        root_manifest,
        message: None,
        stats: converge::model::SnapStats::default(),
    };

    client
        .put(format!(
            "{}/repos/test/objects/snaps/{}",
            base_url1, snap_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&snap)
        .send()
        .context("put snap")?
        .error_for_status()
        .context("put snap status")?;

    // Create publication.
    let pubrec: serde_json::Value = client
        .post(format!("{}/repos/test/publications", base_url1))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap.id,
            "scope": "main",
            "gate": "dev-intake"
        }))
        .send()
        .context("create publication")?
        .error_for_status()
        .context("create publication status")?
        .json()
        .context("parse publication")?;
    let pub_id = pubrec
        .get("id")
        .and_then(|v| v.as_str())
        .context("publication id missing")?
        .to_string();

    // Create bundle.
    client
        .post(format!("{}/repos/test/bundles", base_url1))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pub_id]
        }))
        .send()
        .context("create bundle")?
        .error_for_status()
        .context("create bundle status")?;

    // Verify state visible.
    let pubs1: Vec<serde_json::Value> = client
        .get(format!("{}/repos/test/publications", base_url1))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list publications")?
        .error_for_status()
        .context("list publications status")?
        .json()
        .context("parse publications")?;
    assert_eq!(pubs1.len(), 1);

    let bundles1: Vec<serde_json::Value> = client
        .get(format!("{}/repos/test/bundles", base_url1))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list bundles")?
        .error_for_status()
        .context("list bundles status")?
        .json()
        .context("parse bundles")?;
    assert_eq!(bundles1.len(), 1);

    // Restart server.
    let _ = child1.kill();
    let _ = child1.wait();

    let addr2 = data_dir_path.join("addr2.txt");
    let (mut child2, base_url2) = spawn_server(data_dir_path, &addr2)?;

    let pubs2: Vec<serde_json::Value> = client
        .get(format!("{}/repos/test/publications", base_url2))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list publications after restart")?
        .error_for_status()
        .context("list publications after restart status")?
        .json()
        .context("parse publications after restart")?;
    assert_eq!(pubs2.len(), 1);

    let bundles2: Vec<serde_json::Value> = client
        .get(format!("{}/repos/test/bundles", base_url2))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list bundles after restart")?
        .error_for_status()
        .context("list bundles after restart status")?
        .json()
        .context("parse bundles after restart")?;
    assert_eq!(bundles2.len(), 1);

    // Ensure we wrote repo.json.
    assert!(data_dir_path.join("test").join("repo.json").exists());

    let _ = child2.kill();
    let _ = child2.wait();

    Ok(())
}
