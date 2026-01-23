mod common;

use anyhow::{Context, Result};

#[test]
fn upload_integrity_hash_mismatch_rejected() -> Result<()> {
    let server = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();

    // Create repo.
    client
        .post(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"id": "test"}))
        .send()
        .context("create repo")?
        .error_for_status()
        .context("create repo status")?;

    let bytes = b"abc";
    let correct = blake3::hash(bytes).to_hex().to_string();
    let wrong = "0".repeat(64);

    // Wrong blob id should be rejected.
    let resp = client
        .put(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, wrong
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .body(bytes.to_vec())
        .send()
        .context("upload wrong blob")?;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    // Upload correct blob.
    client
        .put(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, correct
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .body(bytes.to_vec())
        .send()
        .context("upload blob")?
        .error_for_status()
        .context("upload blob status")?;

    // Manifest referencing missing blob should be rejected.
    let manifest_missing = converge::model::Manifest {
        version: 1,
        entries: vec![converge::model::ManifestEntry {
            name: "f.txt".to_string(),
            kind: converge::model::ManifestEntryKind::File {
                blob: converge::model::ObjectId("1".repeat(64)),
                mode: 0o100644,
                size: 1,
            },
        }],
    };
    let manifest_bytes = serde_json::to_vec(&manifest_missing).context("serialize manifest")?;
    let manifest_id = blake3::hash(&manifest_bytes).to_hex().to_string();
    let resp = client
        .put(format!(
            "{}/repos/test/objects/manifests/{}",
            server.base_url, manifest_id
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .body(manifest_bytes)
        .send()
        .context("upload manifest with missing blob")?;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    // Snap id mismatch should be rejected.
    let root_manifest = converge::model::ObjectId(blake3::hash(b"{}").to_hex().to_string());
    let created_at = "2026-01-22T00:00:00Z";
    let snap_id = converge::model::compute_snap_id(created_at, &root_manifest);
    let snap = converge::model::SnapRecord {
        version: 1,
        id: snap_id,
        created_at: created_at.to_string(),
        root_manifest,
        message: None,
        stats: converge::model::SnapStats::default(),
    };

    let resp = client
        .put(format!(
            "{}/repos/test/objects/snaps/{}",
            server.base_url,
            "0".repeat(64)
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&snap)
        .send()
        .context("upload snap with mismatched path")?;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    Ok(())
}
