mod common;

use anyhow::{Context, Result};

#[test]
fn lane_heads_can_be_updated_and_survive_gc() -> Result<()> {
    let server = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();
    let auth = common::auth_header(&server.token);

    // Create repo.
    client
        .post(format!("{}/repos", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({"id": "test"}))
        .send()
        .context("create repo")?
        .error_for_status()
        .context("create repo status")?;

    // Upload an empty manifest and snap.
    let manifest = converge::model::Manifest {
        version: 1,
        entries: Vec::new(),
    };
    let manifest_bytes = serde_json::to_vec(&manifest).context("serialize manifest")?;
    let manifest_id = blake3::hash(&manifest_bytes).to_hex().to_string();

    client
        .put(format!(
            "{}/repos/test/objects/manifests/{}",
            server.base_url, manifest_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(manifest_bytes)
        .send()
        .context("put manifest")?
        .error_for_status()
        .context("put manifest status")?;

    let created_at = "2026-01-22T00:00:00Z";
    let root_manifest = converge::model::ObjectId(manifest_id);
    let snap_id = converge::model::compute_snap_id(created_at, &root_manifest);
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
            server.base_url, snap_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&snap)
        .send()
        .context("put snap")?
        .error_for_status()
        .context("put snap status")?;

    // Update lane head.
    let head: serde_json::Value = client
        .post(format!(
            "{}/repos/test/lanes/default/heads/me",
            server.base_url
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({"snap_id": snap_id}))
        .send()
        .context("update lane head")?
        .error_for_status()
        .context("update lane head status")?
        .json()
        .context("parse lane head")?;

    assert_eq!(
        head.get("snap_id").and_then(|v| v.as_str()),
        Some(snap.id.as_str())
    );

    // Read lane head.
    let head2: serde_json::Value = client
        .get(format!(
            "{}/repos/test/lanes/default/heads/dev",
            server.base_url
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get lane head")?
        .error_for_status()
        .context("get lane head status")?
        .json()
        .context("parse lane head")?;
    assert_eq!(
        head2.get("snap_id").and_then(|v| v.as_str()),
        Some(snap.id.as_str())
    );

    // GC should keep lane head snaps.
    client
        .post(format!(
            "{}/repos/test/gc?dry_run=false&prune_metadata=true",
            server.base_url
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("gc repo")?
        .error_for_status()
        .context("gc repo status")?;

    // Snap should still be readable.
    client
        .get(format!(
            "{}/repos/test/objects/snaps/{}",
            server.base_url, snap.id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get snap")?
        .error_for_status()
        .context("get snap status")?;

    // Lane head should still be present.
    client
        .get(format!(
            "{}/repos/test/lanes/default/heads/dev",
            server.base_url
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get lane head after gc")?
        .error_for_status()
        .context("get lane head after gc status")?;

    Ok(())
}
