mod common;

use anyhow::{Context, Result};

#[test]
fn lane_head_history_retains_recent_snaps_and_allows_gc_of_older() -> Result<()> {
    // Keep in sync with converge-server: LANE_HEAD_HISTORY_KEEP_LAST.
    const KEEP_LAST: usize = 5;

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

    // Upload an empty manifest once.
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

    // Create and upload a series of snaps and advance the lane head each time.
    let mut snap_ids = Vec::new();
    for i in 0..(KEEP_LAST + 2) {
        let created_at = format!("2026-01-22T00:00:{:02}Z", i);
        let root_manifest = converge::model::ObjectId(manifest_id.clone());
        let snap_id = converge::model::compute_snap_id(&created_at, &root_manifest);
        let snap = converge::model::SnapRecord {
            version: 1,
            id: snap_id.clone(),
            created_at,
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
            .with_context(|| format!("put snap {}", i))?
            .error_for_status()
            .with_context(|| format!("put snap {} status", i))?;

        client
            .post(format!(
                "{}/repos/test/lanes/default/heads/me",
                server.base_url
            ))
            .header(reqwest::header::AUTHORIZATION, &auth)
            .json(&serde_json::json!({"snap_id": snap_id}))
            .send()
            .with_context(|| format!("update head {}", i))?
            .error_for_status()
            .with_context(|| format!("update head {} status", i))?;

        snap_ids.push(snap_id);
    }

    // GC should keep the last KEEP_LAST snaps referenced by lane head history.
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

    // Oldest should be gone.
    for sid in snap_ids.iter().take(snap_ids.len() - KEEP_LAST) {
        let resp = client
            .get(format!(
                "{}/repos/test/objects/snaps/{}",
                server.base_url, sid
            ))
            .header(reqwest::header::AUTHORIZATION, &auth)
            .send()
            .with_context(|| format!("get old snap {}", sid))?;
        assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
    }

    // Most recent KEEP_LAST should remain.
    for sid in snap_ids.iter().skip(snap_ids.len() - KEEP_LAST) {
        client
            .get(format!(
                "{}/repos/test/objects/snaps/{}",
                server.base_url, sid
            ))
            .header(reqwest::header::AUTHORIZATION, &auth)
            .send()
            .with_context(|| format!("get kept snap {}", sid))?
            .error_for_status()
            .with_context(|| format!("get kept snap {} status", sid))?;
    }

    Ok(())
}
