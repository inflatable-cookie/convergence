mod common;

use std::fs;

use anyhow::{Context, Result};

use converge::remote::Publication;

fn run_converge(cwd: &std::path::Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_converge"))
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
struct Bundle {
    id: String,
    promotable: bool,
    reasons: Vec<String>,
}

#[test]
fn resolved_publication_includes_resolution_metadata() -> Result<()> {
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

    fs::write(ws1.path().join("a.txt"), b"one\n").context("write a.txt ws1")?;
    let snap1 = run_converge(ws1.path(), &["snap"])?;
    run_converge(ws1.path(), &["publish", "--snap-id", &snap1])?;

    fs::write(ws2.path().join("a.txt"), b"two\n").context("write a.txt ws2")?;
    let snap2 = run_converge(ws2.path(), &["snap"])?;
    run_converge(ws2.path(), &["publish", "--snap-id", &snap2])?;

    let bundle_json = run_converge(ws1.path(), &["bundle", "--json"])?;
    let bundle: Bundle = serde_json::from_str(&bundle_json).context("parse bundle")?;
    assert!(!bundle.promotable);
    assert!(bundle.reasons.iter().any(|r| r == "superpositions_present"));

    // Create a resolution (choose variant #1 for all paths), apply+publish.
    run_converge(
        ws1.path(),
        &["resolve", "init", "--bundle-id", &bundle.id, "--force"],
    )?;
    let show = run_converge(
        ws1.path(),
        &["resolve", "show", "--bundle-id", &bundle.id, "--json"],
    )?;
    let show_json: serde_json::Value = serde_json::from_str(&show).context("parse resolve show")?;
    let conflicts = show_json
        .get("conflicts")
        .and_then(|v| v.as_object())
        .context("conflicts missing")?;
    for (path, _vs) in conflicts {
        run_converge(
            ws1.path(),
            &[
                "resolve",
                "pick",
                "--bundle-id",
                &bundle.id,
                "--path",
                path,
                "--variant",
                "1",
            ],
        )?;
    }

    let out = run_converge(
        ws1.path(),
        &[
            "resolve",
            "apply",
            "--bundle-id",
            &bundle.id,
            "--publish",
            "--json",
        ],
    )?;
    let v: serde_json::Value = serde_json::from_str(&out).context("parse resolve apply json")?;
    let pub_id = v
        .get("published_publication_id")
        .and_then(|v| v.as_str())
        .context("published_publication_id missing")?
        .to_string();

    // Verify publication record contains resolution metadata.
    let client = reqwest::blocking::Client::new();
    let pubs: Vec<Publication> = client
        .get(format!("{}/repos/test/publications", base_url))
        .header(reqwest::header::AUTHORIZATION, common::auth_header(&token))
        .send()
        .context("list publications")?
        .error_for_status()
        .context("list publications status")?
        .json()
        .context("parse publications")?;

    let p = pubs
        .iter()
        .find(|p| p.id == pub_id)
        .context("pub not found")?;
    let res = p.resolution.as_ref().context("resolution missing")?;
    assert_eq!(res.bundle_id, bundle.id);
    assert!(!res.root_manifest.is_empty());
    assert!(!res.resolved_root_manifest.is_empty());
    assert!(!res.created_at.is_empty());

    Ok(())
}
