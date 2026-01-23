mod common;

use anyhow::{Context, Result};

#[test]
fn metadata_only_publications_respect_gate_policy_and_object_availability() -> Result<()> {
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

    // Upload a manifest that references a missing blob, using allow_missing_blobs.
    let missing_blob = "1".repeat(64);
    let manifest = converge::model::Manifest {
        version: 1,
        entries: vec![converge::model::ManifestEntry {
            name: "f.txt".to_string(),
            kind: converge::model::ManifestEntryKind::File {
                blob: converge::model::ObjectId(missing_blob.clone()),
                mode: 0o100644,
                size: 1,
            },
        }],
    };
    let manifest_bytes = serde_json::to_vec(&manifest).context("serialize manifest")?;
    let manifest_id = blake3::hash(&manifest_bytes).to_hex().to_string();

    client
        .put(format!(
            "{}/repos/test/objects/manifests/{}?allow_missing_blobs=true",
            server.base_url, manifest_id
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

    // Default gate graph should reject metadata-only publications.
    let resp = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap.id.clone(),
            "scope": "main",
            "gate": "dev-intake",
            "metadata_only": true
        }))
        .send()
        .context("create metadata-only publication")?;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    // Non-metadata-only publications should require referenced blob bytes.
    let resp = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap.id.clone(),
            "scope": "main",
            "gate": "dev-intake"
        }))
        .send()
        .context("create publication requiring blobs")?;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    // Enable metadata-only publications for dev-intake.
    let mut graph: serde_json::Value = client
        .get(format!("{}/repos/test/gate-graph", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get gate graph")?
        .error_for_status()
        .context("get gate graph status")?
        .json()
        .context("parse gate graph")?;

    let gates = graph
        .get_mut("gates")
        .and_then(|v| v.as_array_mut())
        .context("gate graph gates missing")?;
    for g in gates.iter_mut() {
        if g.get("id") == Some(&serde_json::Value::String("dev-intake".to_string())) {
            g["allow_metadata_only_publications"] = serde_json::Value::Bool(true);
        }
    }

    client
        .put(format!("{}/repos/test/gate-graph", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&graph)
        .send()
        .context("put gate graph")?
        .error_for_status()
        .context("put gate graph status")?;

    // Metadata-only publication should now succeed even though the blob is missing.
    let resp: serde_json::Value = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap.id.clone(),
            "scope": "main",
            "gate": "dev-intake",
            "metadata_only": true
        }))
        .send()
        .context("create metadata-only publication allowed")?
        .error_for_status()
        .context("create metadata-only publication status")?
        .json()
        .context("parse publication")?;
    assert!(resp.get("id").and_then(|v| v.as_str()).is_some());

    Ok(())
}
