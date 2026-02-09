use super::super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_fetch_command(
    ws: &Workspace,
    snap_id: Option<String>,
    bundle_id: Option<String>,
    release: Option<String>,
    lane: Option<String>,
    user: Option<String>,
    restore: bool,
    into: Option<String>,
    force: bool,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    if let Some(bundle_id) = bundle_id.as_deref() {
        let bundle = client.get_bundle(bundle_id)?;
        let root = converge::model::ObjectId(bundle.root_manifest.clone());
        client.fetch_manifest_tree(&ws.store, &root)?;

        let mut restored_to: Option<String> = None;
        if restore {
            let dest = if let Some(p) = into.as_deref() {
                std::path::PathBuf::from(p)
            } else {
                default_temp_destination("converge-grab-bundle", bundle_id)
            };

            ws.materialize_manifest_to(&root, &dest, force)
                .with_context(|| format!("materialize bundle to {}", dest.display()))?;
            restored_to = Some(dest.display().to_string());
            if !json {
                println!("Materialized bundle {} into {}", bundle_id, dest.display());
            }
        }

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "kind": "bundle",
                    "bundle_id": bundle.id,
                    "root_manifest": bundle.root_manifest,
                    "restored_to": restored_to,
                }))
                .context("serialize fetch bundle json")?
            );
        } else {
            println!("Fetched bundle {}", bundle.id);
        }
        return Ok(());
    }

    if let Some(channel) = release.as_deref() {
        let rel = client.get_release(channel)?;
        let bundle = client.get_bundle(&rel.bundle_id)?;
        let root = converge::model::ObjectId(bundle.root_manifest.clone());
        client.fetch_manifest_tree(&ws.store, &root)?;

        let mut restored_to: Option<String> = None;
        if restore {
            let dest = if let Some(p) = into.as_deref() {
                std::path::PathBuf::from(p)
            } else {
                default_temp_destination("converge-grab-release", &rel.bundle_id)
            };

            ws.materialize_manifest_to(&root, &dest, force)
                .with_context(|| format!("materialize release to {}", dest.display()))?;
            restored_to = Some(dest.display().to_string());
            if !json {
                println!(
                    "Materialized release {} (bundle {}) into {}",
                    rel.channel,
                    rel.bundle_id,
                    dest.display()
                );
            }
        }

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "kind": "release",
                    "channel": rel.channel,
                    "bundle_id": rel.bundle_id,
                    "root_manifest": bundle.root_manifest,
                    "restored_to": restored_to,
                }))
                .context("serialize fetch release json")?
            );
        } else {
            println!("Fetched release {} ({})", rel.channel, rel.bundle_id);
        }
        return Ok(());
    }

    let fetched = if let Some(lane) = lane.as_deref() {
        client.fetch_lane_heads(&ws.store, lane, user.as_deref())?
    } else {
        client.fetch_publications(&ws.store, snap_id.as_deref())?
    };

    if restore {
        let snap_to_restore = if let Some(id) = snap_id.as_deref() {
            id.to_string()
        } else if fetched.len() == 1 {
            fetched[0].clone()
        } else {
            anyhow::bail!(
                "--restore requires a specific snap (use --snap-id, or use --user so only one lane head is fetched)"
            );
        };

        let dest = if let Some(p) = into.as_deref() {
            std::path::PathBuf::from(p)
        } else {
            default_temp_destination("converge-grab", &snap_to_restore)
        };

        ws.materialize_snap_to(&snap_to_restore, &dest, force)
            .with_context(|| format!("materialize snap to {}", dest.display()))?;
        if !json {
            println!("Materialized {} into {}", snap_to_restore, dest.display());
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&fetched).context("serialize fetch json")?
        );
    } else {
        for id in fetched {
            println!("Fetched {}", id);
        }
    }

    Ok(())
}

fn default_temp_destination(prefix: &str, id: &str) -> std::path::PathBuf {
    let short = id.chars().take(8).collect::<String>();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("{}-{}-{}", prefix, short, nanos))
}
