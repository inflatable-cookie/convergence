use super::*;

pub(super) fn handle_approve_command(ws: &Workspace, bundle_id: String, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let bundle = client.approve_bundle(&bundle_id)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&bundle).context("serialize approve json")?
        );
    } else if bundle.promotable {
        println!("Approved {} (now promotable)", bundle.id);
    } else {
        println!(
            "Approved {} (still blocked: {:?})",
            bundle.id, bundle.reasons
        );
    }

    Ok(())
}

pub(super) fn handle_pins_command(ws: &Workspace, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let pins = client.list_pins()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&pins).context("serialize pins json")?
        );
    } else {
        for b in pins.bundles {
            println!("{}", b);
        }
    }

    Ok(())
}

pub(super) fn handle_pin_command(
    ws: &Workspace,
    bundle_id: String,
    unpin: bool,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    if unpin {
        client.unpin_bundle(&bundle_id)?;
    } else {
        client.pin_bundle(&bundle_id)?;
    }
    if json {
        println!(
            "{}",
            serde_json::json!({
                "bundle_id": bundle_id,
                "pinned": !unpin
            })
        );
    } else if unpin {
        println!("Unpinned {}", bundle_id);
    } else {
        println!("Pinned {}", bundle_id);
    }

    Ok(())
}

pub(super) fn handle_status_command(ws: &Workspace, json: bool, limit: usize) -> Result<()> {
    let cfg = ws.store.read_config()?;
    let Some(remote) = cfg.remote else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({"remote": null}))
                    .context("serialize status json")?
            );
        } else {
            println!("No remote configured");
        }
        return Ok(());
    };

    let token = ws.store.get_remote_token(&remote)?.context(
        "no remote token configured (run `converge login --url ... --token ... --repo ...`)",
    )?;
    let client = RemoteClient::new(remote.clone(), token)?;
    let mut pubs = client.list_publications()?;
    pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    pubs.truncate(limit);
    let promotion_state = client.promotion_state(&remote.scope)?;
    let releases = client.list_releases().unwrap_or_default();

    let mut latest_by_channel: std::collections::BTreeMap<String, converge::remote::Release> =
        std::collections::BTreeMap::new();
    for r in releases {
        match latest_by_channel.get(&r.channel) {
            None => {
                latest_by_channel.insert(r.channel.clone(), r);
            }
            Some(prev) => {
                if r.released_at > prev.released_at {
                    latest_by_channel.insert(r.channel.clone(), r);
                }
            }
        }
    }

    if json {
        let remote_json = serde_json::json!({
            "base_url": remote.base_url.as_str(),
            "repo_id": remote.repo_id.as_str(),
            "scope": remote.scope.as_str(),
            "gate": remote.gate.as_str(),
        });
        let pubs_json = pubs
            .into_iter()
            .map(|p| {
                let present = ws.store.has_snap(&p.snap_id);
                serde_json::json!({
                    "id": p.id,
                    "snap_id": p.snap_id,
                    "scope": p.scope,
                    "gate": p.gate,
                    "publisher": p.publisher,
                    "created_at": p.created_at,
                    "local_present": present
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "remote": remote_json,
                "publications": pubs_json,
                "promotion_state": promotion_state,
                "releases": latest_by_channel.values().collect::<Vec<_>>()
            }))
            .context("serialize status json")?
        );
    } else {
        println!("remote: {}", remote.base_url);
        println!("repo: {}", remote.repo_id);
        println!("scope: {}", remote.scope);
        println!("gate: {}", remote.gate);

        println!("releases:");
        if latest_by_channel.is_empty() {
            println!("(none)");
        } else {
            for (ch, r) in &latest_by_channel {
                let short = r.bundle_id.chars().take(8).collect::<String>();
                println!("{} {} {} {}", ch, short, r.released_at, r.released_by);
            }
        }

        println!("promotion_state:");
        if promotion_state.is_empty() {
            println!("(none)");
        } else {
            let mut keys = promotion_state.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for gate in keys {
                let bid = promotion_state.get(&gate).cloned().unwrap_or_default();
                let short = bid.chars().take(8).collect::<String>();
                println!("{} {}", gate, short);
            }
        }
        println!("publications:");
        for p in pubs {
            let short = p.snap_id.chars().take(8).collect::<String>();
            let present = if ws.store.has_snap(&p.snap_id) {
                "local"
            } else {
                "missing"
            };
            println!(
                "{} {} {} {} {}",
                short, p.created_at, p.publisher, p.scope, present
            );
        }
    }

    Ok(())
}

pub(super) fn handle_publish_command(
    ws: &Workspace,
    snap_id: Option<String>,
    scope: Option<String>,
    gate: Option<String>,
    metadata_only: bool,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote.clone(), token)?;

    let snap = match snap_id {
        Some(id) => ws.show_snap(&id)?,
        None => ws
            .list_snaps()?
            .into_iter()
            .next()
            .context("no snaps found (run `converge snap`)")?,
    };

    let scope = scope.unwrap_or_else(|| remote.scope.clone());
    let gate = gate.unwrap_or_else(|| remote.gate.clone());

    let pubrec = if metadata_only {
        client.publish_snap_metadata_only(&ws.store, &snap, &scope, &gate)?
    } else {
        client.publish_snap(&ws.store, &snap, &scope, &gate)?
    };

    ws.store
        .set_last_published(&remote, &scope, &gate, &snap.id)
        .context("record last published snap")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&pubrec).context("serialize publish json")?
        );
    } else {
        println!("Published {}", snap.id);
    }

    Ok(())
}

pub(super) fn handle_sync_command(
    ws: &Workspace,
    snap_id: Option<String>,
    lane: String,
    client_id: Option<String>,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;

    let snap = match snap_id {
        Some(id) => ws.show_snap(&id)?,
        None => ws
            .list_snaps()?
            .into_iter()
            .next()
            .context("no snaps to sync")?,
    };

    let head = client.sync_snap(&ws.store, &snap, &lane, client_id)?;

    ws.store
        .set_lane_sync(&lane, &snap.id, &head.updated_at)
        .context("record lane sync")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&head).context("serialize sync json")?
        );
    } else {
        println!("Synced {} to lane {}", snap.id, lane);
    }

    Ok(())
}

pub(super) fn handle_lanes_command(ws: &Workspace, json: bool) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let mut lanes = client.list_lanes()?;
    lanes.sort_by(|a, b| a.id.cmp(&b.id));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&lanes).context("serialize lanes json")?
        );
    } else {
        for l in lanes {
            println!("lane: {}", l.id);
            let mut members = l.members.into_iter().collect::<Vec<_>>();
            members.sort();
            for m in members {
                if let Some(h) = l.heads.get(&m) {
                    let short = h.snap_id.chars().take(8).collect::<String>();
                    println!("  {} {} {}", m, short, h.updated_at);
                } else {
                    println!("  {} (no head)", m);
                }
            }
        }
    }

    Ok(())
}

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
                let short = bundle_id.chars().take(8).collect::<String>();
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                std::env::temp_dir().join(format!("converge-grab-bundle-{}-{}", short, nanos))
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
                let short = rel.bundle_id.chars().take(8).collect::<String>();
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                std::env::temp_dir().join(format!("converge-grab-release-{}-{}", short, nanos))
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
            let short = snap_to_restore.chars().take(8).collect::<String>();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            std::env::temp_dir().join(format!("converge-grab-{}-{}", short, nanos))
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

pub(super) fn handle_bundle_command(
    ws: &Workspace,
    scope: Option<String>,
    gate: Option<String>,
    publications: Vec<String>,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote.clone(), token)?;
    let scope = scope.unwrap_or_else(|| remote.scope.clone());
    let gate = gate.unwrap_or_else(|| remote.gate.clone());

    let pubs = if publications.is_empty() {
        let all = client.list_publications()?;
        all.into_iter()
            .filter(|p| p.scope == scope && p.gate == gate)
            .map(|p| p.id)
            .collect::<Vec<_>>()
    } else {
        publications
    };

    if pubs.is_empty() {
        anyhow::bail!(
            "no publications found for scope={} gate={} (publish first)",
            scope,
            gate
        );
    }

    let bundle = client.create_bundle(&scope, &gate, &pubs)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&bundle).context("serialize bundle json")?
        );
    } else {
        println!("{}", bundle.id);
    }

    Ok(())
}

pub(super) fn handle_promote_command(
    ws: &Workspace,
    bundle_id: String,
    to_gate: String,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote, token)?;
    let promotion = client.promote_bundle(&bundle_id, &to_gate)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&promotion).context("serialize promotion json")?
        );
    } else {
        println!("Promoted {} -> {}", promotion.from_gate, promotion.to_gate);
    }

    Ok(())
}
