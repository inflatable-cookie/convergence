mod common;

use anyhow::{Context, Result};

#[test]
fn server_api_contract_happy_path_and_auth_failures() -> Result<()> {
    let server = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();

    // Health is unauthenticated.
    let health = client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .context("healthz")?;
    assert!(health.status().is_success());

    // Auth is required for most endpoints.
    let whoami = client
        .get(format!("{}/whoami", server.base_url))
        .send()
        .context("whoami")?;
    assert_eq!(whoami.status(), reqwest::StatusCode::UNAUTHORIZED);

    // Authenticated whoami returns identity.
    let whoami: serde_json::Value = client
        .get(format!("{}/whoami", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("whoami authed")?
        .error_for_status()
        .context("whoami authed status")?
        .json()
        .context("parse whoami")?;
    assert_eq!(
        whoami.get("user"),
        Some(&serde_json::Value::String("dev".to_string()))
    );
    assert!(whoami.get("user_id").and_then(|v| v.as_str()).is_some());

    // Create repo.
    let created = client
        .post(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"id": "test"}))
        .send()
        .context("create repo")?;
    assert!(created.status().is_success());

    // List repos.
    let repos: serde_json::Value = client
        .get(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("list repos")?
        .error_for_status()
        .context("list repos status")?
        .json()
        .context("parse repos")?;

    assert!(repos.is_array());
    assert!(
        repos
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.get("id") == Some(&serde_json::Value::String("test".to_string())))
    );

    // Invalid repo id rejected.
    let bad = client
        .post(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"id": "Bad"}))
        .send()
        .context("create repo invalid")?;
    assert_eq!(bad.status(), reqwest::StatusCode::BAD_REQUEST);

    // Unknown repo.
    let missing = client
        .get(format!("{}/repos/nope", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("get missing repo")?;
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    // Releases endpoints require auth.
    let rels = client
        .get(format!("{}/repos/test/releases", server.base_url))
        .send()
        .context("list releases unauth")?;
    assert_eq!(rels.status(), reqwest::StatusCode::UNAUTHORIZED);

    // GC endpoint requires auth.
    let gc = client
        .post(format!("{}/repos/test/gc", server.base_url))
        .send()
        .context("gc unauth")?;
    assert_eq!(gc.status(), reqwest::StatusCode::UNAUTHORIZED);

    // Configure metadata-only publications for dev-intake.
    let mut graph: serde_json::Value = client
        .get(format!("{}/repos/test/gate-graph", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
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
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&graph)
        .send()
        .context("put gate graph")?
        .error_for_status()
        .context("put gate graph status")?;

    // Upload a manifest and snap referencing a missing blob.
    let missing_blob = "1".repeat(64);
    let manifest = converge::model::Manifest {
        version: 1,
        entries: vec![converge::model::ManifestEntry {
            name: "f.txt".to_string(),
            kind: converge::model::ManifestEntryKind::File {
                blob: converge::model::ObjectId(missing_blob),
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
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .body(manifest_bytes)
        .send()
        .context("put manifest")?
        .error_for_status()
        .context("put manifest status")?;

    let created_at = "2026-01-25T00:00:00Z";
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
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&snap)
        .send()
        .context("put snap")?
        .error_for_status()
        .context("put snap status")?;

    // Create metadata-only publication.
    let pubrec: serde_json::Value = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({
            "snap_id": snap_id,
            "scope": "main",
            "gate": "dev-intake",
            "metadata_only": true
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
        .context("missing pub id")?
        .to_string();

    // Publishing the same snap+scope+gate twice should be rejected.
    let dup = client
        .post(format!("{}/repos/test/publications", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({
            "snap_id": snap_id,
            "scope": "main",
            "gate": "dev-intake",
            "metadata_only": true
        }))
        .send()
        .context("create duplicate publication")?;
    assert_eq!(dup.status(), reqwest::StatusCode::CONFLICT);

    // Create bundle.
    let bundle: serde_json::Value = client
        .post(format!("{}/repos/test/bundles", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({
            "scope": "main",
            "gate": "dev-intake",
            "input_publications": [pub_id]
        }))
        .send()
        .context("create bundle")?
        .error_for_status()
        .context("create bundle status")?
        .json()
        .context("parse bundle")?;
    let bundle_id = bundle
        .get("id")
        .and_then(|v| v.as_str())
        .context("missing bundle id")?
        .to_string();

    // Create and fetch a release.
    let rel: serde_json::Value = client
        .post(format!("{}/repos/test/releases", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"channel": "stable", "bundle_id": bundle_id}))
        .send()
        .context("create release")?
        .error_for_status()
        .context("create release status")?
        .json()
        .context("parse release")?;
    assert_eq!(
        rel.get("channel"),
        Some(&serde_json::Value::String("stable".to_string()))
    );

    // GC dry run shape.
    let gc: serde_json::Value = client
        .post(format!(
            "{}/repos/test/gc?dry_run=true&prune_metadata=true",
            server.base_url
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("gc")?
        .error_for_status()
        .context("gc status")?
        .json()
        .context("parse gc")?;
    assert_eq!(gc.get("dry_run"), Some(&serde_json::Value::Bool(true)));
    assert_eq!(
        gc.get("prune_metadata"),
        Some(&serde_json::Value::Bool(true))
    );
    assert!(gc.get("kept").is_some());
    assert!(gc.get("deleted").is_some());

    let rels: serde_json::Value = client
        .get(format!("{}/repos/test/releases", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("list releases")?
        .error_for_status()
        .context("list releases status")?
        .json()
        .context("parse releases")?;
    assert!(rels.as_array().is_some());

    let rel: serde_json::Value = client
        .get(format!("{}/repos/test/releases/stable", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("get release")?
        .error_for_status()
        .context("get release status")?
        .json()
        .context("parse release")?;
    assert_eq!(
        rel.get("channel"),
        Some(&serde_json::Value::String("stable".to_string()))
    );

    Ok(())
}
