use super::*;

pub(super) fn handle_resolve_apply(
    ws: &Workspace,
    client: &RemoteClient,
    input: ResolveApplyInput,
) -> Result<()> {
    let bundle = client.get_bundle(&input.bundle_id)?;

    // Ensure we can read manifests/blobs needed for applying resolution.
    let root = converge::model::ObjectId(bundle.root_manifest.clone());
    client.fetch_manifest_tree(&ws.store, &root)?;

    let resolution = ws.store.get_resolution(&input.bundle_id)?;
    if resolution.root_manifest != root {
        anyhow::bail!(
            "resolution root_manifest mismatch (resolution {}, bundle {})",
            resolution.root_manifest.as_str(),
            root.as_str()
        );
    }

    let resolved_root =
        converge::resolve::apply_resolution(&ws.store, &root, &resolution.decisions)?;

    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .context("format time")?;
    let snap_id = converge::model::compute_snap_id(&created_at, &resolved_root);

    let snap = converge::model::SnapRecord {
        version: 1,
        id: snap_id,
        created_at,
        root_manifest: resolved_root,
        message: input.message,
        stats: converge::model::SnapStats::default(),
    };

    ws.store.put_snap(&snap)?;

    let mut pub_id = None;
    if input.publish {
        let pubrec = client.publish_snap_with_resolution(
            &ws.store,
            &snap,
            &input.scope,
            &input.gate,
            Some(converge::remote::PublicationResolution {
                bundle_id: input.bundle_id.clone(),
                root_manifest: root.as_str().to_string(),
                resolved_root_manifest: snap.root_manifest.as_str().to_string(),
                created_at: snap.created_at.clone(),
            }),
        )?;
        pub_id = Some(pubrec.id);
    }

    if input.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "snap": snap,
                "published_publication_id": pub_id
            }))
            .context("serialize resolve json")?
        );
    } else {
        println!("Resolved snap {}", snap.id);
        if let Some(pid) = pub_id {
            println!("Published {}", pid);
        }
    }

    Ok(())
}
