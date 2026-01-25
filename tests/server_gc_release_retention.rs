use anyhow::{Context, Result};

mod common;

#[test]
fn server_gc_retains_released_bundles_and_objects() -> Result<()> {
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

    // Upload two blobs.
    let blob1_bytes = b"a".to_vec();
    let blob2_bytes = b"b".to_vec();
    let blob1_id = blake3::hash(&blob1_bytes).to_hex().to_string();
    let blob2_id = blake3::hash(&blob2_bytes).to_hex().to_string();

    client
        .put(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, blob1_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(blob1_bytes)
        .send()
        .context("put blob1")?
        .error_for_status()
        .context("put blob1 status")?;

    client
        .put(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, blob2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(blob2_bytes)
        .send()
        .context("put blob2")?
        .error_for_status()
        .context("put blob2 status")?;

    // Upload two manifests, each referencing one blob.
    let manifest1 = converge::model::Manifest {
        version: 1,
        entries: vec![converge::model::ManifestEntry {
            name: "a.txt".to_string(),
            kind: converge::model::ManifestEntryKind::File {
                blob: converge::model::ObjectId(blob1_id.clone()),
                mode: 0o100644,
                size: 1,
            },
        }],
    };
    let manifest2 = converge::model::Manifest {
        version: 1,
        entries: vec![converge::model::ManifestEntry {
            name: "b.txt".to_string(),
            kind: converge::model::ManifestEntryKind::File {
                blob: converge::model::ObjectId(blob2_id.clone()),
                mode: 0o100644,
                size: 1,
            },
        }],
    };
    let manifest1_bytes = serde_json::to_vec(&manifest1).context("serialize manifest1")?;
    let manifest2_bytes = serde_json::to_vec(&manifest2).context("serialize manifest2")?;
    let manifest1_id = blake3::hash(&manifest1_bytes).to_hex().to_string();
    let manifest2_id = blake3::hash(&manifest2_bytes).to_hex().to_string();

    client
        .put(format!(
            "{}/repos/test/objects/manifests/{}",
            server.base_url, manifest1_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(manifest1_bytes)
        .send()
        .context("put manifest1")?
        .error_for_status()
        .context("put manifest1 status")?;

    client
        .put(format!(
            "{}/repos/test/objects/manifests/{}",
            server.base_url, manifest2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .body(manifest2_bytes)
        .send()
        .context("put manifest2")?
        .error_for_status()
        .context("put manifest2 status")?;

    // Upload two snaps.
    let created1 = "2026-01-22T00:00:00Z";
    let created2 = "2026-01-22T00:00:01Z";
    let root1 = converge::model::ObjectId(manifest1_id.clone());
    let root2 = converge::model::ObjectId(manifest2_id.clone());
    let snap1_id = converge::model::compute_snap_id(created1, &root1);
    let snap2_id = converge::model::compute_snap_id(created2, &root2);

    let snap1 = converge::model::SnapRecord {
        version: 1,
        id: snap1_id.clone(),
        created_at: created1.to_string(),
        root_manifest: root1,
        message: None,
        stats: converge::model::SnapStats::default(),
    };
    let snap2 = converge::model::SnapRecord {
        version: 1,
        id: snap2_id.clone(),
        created_at: created2.to_string(),
        root_manifest: root2,
        message: None,
        stats: converge::model::SnapStats::default(),
    };

    client
        .put(format!(
            "{}/repos/test/objects/snaps/{}",
            server.base_url, snap1_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&snap1)
        .send()
        .context("put snap1")?
        .error_for_status()
        .context("put snap1 status")?;

    client
        .put(format!(
            "{}/repos/test/objects/snaps/{}",
            server.base_url, snap2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&snap2)
        .send()
        .context("put snap2")?
        .error_for_status()
        .context("put snap2 status")?;

    // Create two publications.
    let pub1: serde_json::Value = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap1_id,
            "scope": "main",
            "gate": "dev-intake"
        }))
        .send()
        .context("create publication1")?
        .error_for_status()
        .context("create publication1 status")?
        .json()
        .context("parse publication1")?;
    let pub1_id = pub1
        .get("id")
        .and_then(|v| v.as_str())
        .context("pub1 id missing")?
        .to_string();

    let pub2: serde_json::Value = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "snap_id": snap2_id,
            "scope": "main",
            "gate": "dev-intake"
        }))
        .send()
        .context("create publication2")?
        .error_for_status()
        .context("create publication2 status")?
        .json()
        .context("parse publication2")?;
    let pub2_id = pub2
        .get("id")
        .and_then(|v| v.as_str())
        .context("pub2 id missing")?
        .to_string();

    // Create two bundles, one per publication.
    let bundle1: serde_json::Value = client
        .post(format!("{}/repos/test/bundles", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pub1_id]
        }))
        .send()
        .context("create bundle1")?
        .error_for_status()
        .context("create bundle1 status")?
        .json()
        .context("parse bundle1")?;
    let bundle1_id = bundle1
        .get("id")
        .and_then(|v| v.as_str())
        .context("bundle1 id missing")?
        .to_string();

    let bundle2: serde_json::Value = client
        .post(format!("{}/repos/test/bundles", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pub2_id]
        }))
        .send()
        .context("create bundle2")?
        .error_for_status()
        .context("create bundle2 status")?
        .json()
        .context("parse bundle2")?;
    let bundle2_id = bundle2
        .get("id")
        .and_then(|v| v.as_str())
        .context("bundle2 id missing")?
        .to_string();

    // Release bundle1.
    client
        .post(format!("{}/repos/test/releases", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({"channel": "stable", "bundle_id": bundle1_id}))
        .send()
        .context("create release")?
        .error_for_status()
        .context("create release status")?;

    // Run server GC: keep released bundle + its provenance, prune everything else.
    let report: serde_json::Value = client
        .post(format!(
            "{}/repos/test/gc?dry_run=false&prune_metadata=true",
            server.base_url
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("gc repo")?
        .error_for_status()
        .context("gc repo status")?
        .json()
        .context("parse gc report")?;
    assert_eq!(report.get("dry_run"), Some(&serde_json::Value::Bool(false)));

    // Bundle2 should be gone.
    let resp = client
        .get(format!(
            "{}/repos/test/bundles/{}",
            server.base_url, bundle2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get bundle2")?;
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    // blob2 + manifest2 + snap2 should be gone.
    let resp = client
        .get(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, blob2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get blob2")?;
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    let resp = client
        .get(format!(
            "{}/repos/test/objects/manifests/{}",
            server.base_url, manifest2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get manifest2")?;
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    let resp = client
        .get(format!(
            "{}/repos/test/objects/snaps/{}",
            server.base_url, snap2_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get snap2")?;
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    // blob1 should remain.
    let resp = client
        .get(format!(
            "{}/repos/test/objects/blobs/{}",
            server.base_url, blob1_id
        ))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get blob1")?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // Release should remain.
    let resp = client
        .get(format!("{}/repos/test/releases/stable", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("get release")?;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    Ok(())
}
