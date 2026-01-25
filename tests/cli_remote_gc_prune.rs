use std::process::Command;

use anyhow::{Context, Result};

mod common;

fn run_converge(cwd: &std::path::Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(cwd)
        .args(args)
        .output()
        .with_context(|| format!("run converge {:?}", args))
}

fn ensure_ok(label: &str, out: &std::process::Output) -> Result<()> {
    if out.status.success() {
        return Ok(());
    }
    anyhow::bail!(
        "{} failed\nstdout:\n{}\nstderr:\n{}",
        label,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn cli_remote_gc_can_prune_release_history() -> Result<()> {
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

    // Two blobs.
    let blob1 = b"a".to_vec();
    let blob2 = b"b".to_vec();
    let blob1_id = blake3::hash(&blob1).to_hex().to_string();
    let blob2_id = blake3::hash(&blob2).to_hex().to_string();
    for (id, bytes) in [(blob1_id.clone(), blob1), (blob2_id.clone(), blob2)] {
        client
            .put(format!(
                "{}/repos/test/objects/blobs/{}",
                server.base_url, id
            ))
            .header(reqwest::header::AUTHORIZATION, &auth)
            .body(bytes)
            .send()
            .with_context(|| format!("put blob {}", id))?
            .error_for_status()
            .with_context(|| format!("put blob {} status", id))?;
    }

    // Two manifests/snaps/pubs/bundles.
    fn mk_manifest(name: &str, blob: &str) -> converge::model::Manifest {
        converge::model::Manifest {
            version: 1,
            entries: vec![converge::model::ManifestEntry {
                name: name.to_string(),
                kind: converge::model::ManifestEntryKind::File {
                    blob: converge::model::ObjectId(blob.to_string()),
                    mode: 0o100644,
                    size: 1,
                },
            }],
        }
    }

    fn put_manifest(
        client: &reqwest::blocking::Client,
        base_url: &str,
        auth: &str,
        m: &converge::model::Manifest,
    ) -> Result<String> {
        let bytes = serde_json::to_vec(m).context("serialize manifest")?;
        let id = blake3::hash(&bytes).to_hex().to_string();
        client
            .put(format!("{}/repos/test/objects/manifests/{}", base_url, id))
            .header(reqwest::header::AUTHORIZATION, auth)
            .body(bytes)
            .send()
            .context("put manifest")?
            .error_for_status()
            .context("put manifest status")?;
        Ok(id)
    }

    fn put_snap(
        client: &reqwest::blocking::Client,
        base_url: &str,
        auth: &str,
        created_at: &str,
        root_manifest: &str,
    ) -> Result<String> {
        let root = converge::model::ObjectId(root_manifest.to_string());
        let snap_id = converge::model::compute_snap_id(created_at, &root);
        let snap = converge::model::SnapRecord {
            version: 1,
            id: snap_id.clone(),
            created_at: created_at.to_string(),
            root_manifest: root,
            message: None,
            stats: converge::model::SnapStats::default(),
        };
        client
            .put(format!("{}/repos/test/objects/snaps/{}", base_url, snap_id))
            .header(reqwest::header::AUTHORIZATION, auth)
            .json(&snap)
            .send()
            .context("put snap")?
            .error_for_status()
            .context("put snap status")?;
        Ok(snap_id)
    }

    fn create_pub(
        client: &reqwest::blocking::Client,
        base_url: &str,
        auth: &str,
        snap_id: &str,
    ) -> Result<String> {
        let pubrec: serde_json::Value = client
            .post(format!("{}/repos/test/publications", base_url))
            .header(reqwest::header::AUTHORIZATION, auth)
            .json(&serde_json::json!({
                "snap_id": snap_id,
                "scope": "main",
                "gate": "dev-intake"
            }))
            .send()
            .context("create pub")?
            .error_for_status()
            .context("create pub status")?
            .json()
            .context("parse pub")?;
        Ok(pubrec
            .get("id")
            .and_then(|v| v.as_str())
            .context("pub id missing")?
            .to_string())
    }

    fn create_bundle(
        client: &reqwest::blocking::Client,
        base_url: &str,
        auth: &str,
        pub_id: &str,
    ) -> Result<String> {
        let b: serde_json::Value = client
            .post(format!("{}/repos/test/bundles", base_url))
            .header(reqwest::header::AUTHORIZATION, auth)
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
        Ok(b.get("id")
            .and_then(|v| v.as_str())
            .context("bundle id missing")?
            .to_string())
    }

    fn create_release(
        client: &reqwest::blocking::Client,
        base_url: &str,
        auth: &str,
        bundle_id: &str,
    ) -> Result<()> {
        client
            .post(format!("{}/repos/test/releases", base_url))
            .header(reqwest::header::AUTHORIZATION, auth)
            .json(&serde_json::json!({"channel": "stable", "bundle_id": bundle_id}))
            .send()
            .context("create release")?
            .error_for_status()
            .context("create release status")?;
        Ok(())
    }

    let m1 = mk_manifest("a.txt", &blob1_id);
    let m2 = mk_manifest("b.txt", &blob2_id);
    let mid1 = put_manifest(&client, &server.base_url, &auth, &m1)?;
    let mid2 = put_manifest(&client, &server.base_url, &auth, &m2)?;
    let s1 = put_snap(
        &client,
        &server.base_url,
        &auth,
        "2026-01-25T00:00:00Z",
        &mid1,
    )?;
    let s2 = put_snap(
        &client,
        &server.base_url,
        &auth,
        "2026-01-25T00:00:01Z",
        &mid2,
    )?;
    let p1 = create_pub(&client, &server.base_url, &auth, &s1)?;
    let p2 = create_pub(&client, &server.base_url, &auth, &s2)?;
    let b1 = create_bundle(&client, &server.base_url, &auth, &p1)?;
    let b2 = create_bundle(&client, &server.base_url, &auth, &p2)?;
    create_release(&client, &server.base_url, &auth, &b1)?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    create_release(&client, &server.base_url, &auth, &b2)?;

    // Workspace for CLI.
    let ws = tempfile::tempdir().context("create ws")?;
    ensure_ok("init", &run_converge(ws.path(), &["init"])?)?;
    ensure_ok(
        "login",
        &run_converge(
            ws.path(),
            &[
                "login",
                "--url",
                &server.base_url,
                "--token",
                &server.token,
                "--repo",
                "test",
            ],
        )?,
    )?;

    // Prune old releases.
    let out = run_converge(
        ws.path(),
        &[
            "remote",
            "gc",
            "--dry-run",
            "false",
            "--prune-metadata",
            "true",
            "--prune-releases-keep-last",
            "1",
            "--json",
        ],
    )?;
    ensure_ok("remote gc", &out)?;

    // Only one release remains.
    let rels: Vec<serde_json::Value> = client
        .get(format!("{}/repos/test/releases", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list releases")?
        .error_for_status()
        .context("list releases status")?
        .json()
        .context("parse releases")?;
    assert_eq!(rels.len(), 1);

    Ok(())
}
