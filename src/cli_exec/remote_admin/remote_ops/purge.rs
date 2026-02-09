use super::*;

pub(super) fn purge_remote(
    ws: &Workspace,
    dry_run: bool,
    prune_metadata: bool,
    prune_releases_keep_last: Option<usize>,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let report = client.gc_repo(dry_run, prune_metadata, prune_releases_keep_last)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).context("serialize purge json")?
        );
    } else {
        let kept = report.get("kept").and_then(|v| v.as_object());
        let deleted = report.get("deleted").and_then(|v| v.as_object());
        println!("dry_run: {}", dry_run);
        println!("prune_metadata: {}", prune_metadata);
        if let Some(n) = prune_releases_keep_last {
            println!("prune_releases_keep_last: {}", n);
        }
        if let Some(k) = kept {
            println!(
                "kept: bundles={} releases={} snaps={} blobs={} manifests={} recipes={}",
                k.get("bundles").and_then(|v| v.as_u64()).unwrap_or(0),
                k.get("releases").and_then(|v| v.as_u64()).unwrap_or(0),
                k.get("snaps").and_then(|v| v.as_u64()).unwrap_or(0),
                k.get("blobs").and_then(|v| v.as_u64()).unwrap_or(0),
                k.get("manifests").and_then(|v| v.as_u64()).unwrap_or(0),
                k.get("recipes").and_then(|v| v.as_u64()).unwrap_or(0),
            );
        }
        if let Some(d) = deleted {
            println!(
                "deleted: bundles={} releases={} snaps={} blobs={} manifests={} recipes={}",
                d.get("bundles").and_then(|v| v.as_u64()).unwrap_or(0),
                d.get("releases").and_then(|v| v.as_u64()).unwrap_or(0),
                d.get("snaps").and_then(|v| v.as_u64()).unwrap_or(0),
                d.get("blobs").and_then(|v| v.as_u64()).unwrap_or(0),
                d.get("manifests").and_then(|v| v.as_u64()).unwrap_or(0),
                d.get("recipes").and_then(|v| v.as_u64()).unwrap_or(0),
            );
        }
    }

    Ok(())
}
