use super::super::*;

pub(super) fn handle_resolve_clear(
    ws: &Workspace,
    bundle_id: String,
    path: String,
    json: bool,
) -> Result<()> {
    let mut r = ws.store.get_resolution(&bundle_id)?;
    r.decisions.remove(&path);
    if r.version == 1 {
        r.version = 2;
    }
    ws.store.put_resolution(&r)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&r).context("serialize resolution")?
        );
    } else {
        println!("Cleared decision for {}", path);
    }

    Ok(())
}

pub(super) fn handle_resolve_show(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    json: bool,
) -> Result<()> {
    let r = ws.store.get_resolution(&bundle_id)?;

    // Best-effort fetch so we can enumerate current conflicts.
    let _ = client.fetch_manifest_tree(&ws.store, &r.root_manifest);

    let variants =
        converge::resolve::superposition_variants(&ws.store, &r.root_manifest).unwrap_or_default();
    let decided = variants
        .keys()
        .filter(|p| r.decisions.contains_key(*p))
        .count();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "resolution": r,
                "conflicts": variants,
                "decided": decided
            }))
            .context("serialize resolve show json")?
        );
    } else {
        println!("bundle: {}", r.bundle_id);
        println!("root_manifest: {}", r.root_manifest.as_str());
        println!("created_at: {}", r.created_at);
        println!("decisions: {}", r.decisions.len());

        if !variants.is_empty() {
            println!("decided: {}/{}", decided, variants.len());
            println!("conflicts:");
            for (p, vs) in variants {
                println!("{} (variants: {})", p, vs.len());
                for (idx, v) in vs.iter().enumerate() {
                    let n = idx + 1;
                    let key_json =
                        serde_json::to_string(&v.key()).context("serialize variant key")?;
                    println!("  #{} source={}", n, v.source);
                    println!("    key={}", key_json);
                }
            }
        }
    }

    Ok(())
}
