use super::*;

pub(super) fn handle_resolve_validate(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    json: bool,
) -> Result<()> {
    let bundle = client.get_bundle(&bundle_id)?;
    let root = converge::model::ObjectId(bundle.root_manifest.clone());
    client.fetch_manifest_tree(&ws.store, &root)?;

    let r = ws.store.get_resolution(&bundle_id)?;
    if r.root_manifest != root {
        anyhow::bail!(
            "resolution root_manifest mismatch (resolution {}, bundle {})",
            r.root_manifest.as_str(),
            root.as_str()
        );
    }

    let report = converge::resolve::validate_resolution(&ws.store, &root, &r.decisions)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "bundle_id": bundle_id,
                "root_manifest": root,
                "report": report,
            }))
            .context("serialize resolve validate json")?
        );
    } else {
        if report.ok {
            println!("OK");
        } else {
            println!("Invalid");
        }
        if !report.missing.is_empty() {
            println!("missing:");
            for p in &report.missing {
                println!("{}", p);
            }
        }
        if !report.out_of_range.is_empty() {
            println!("out_of_range:");
            for d in &report.out_of_range {
                println!("{} index={} variants={}", d.path, d.index, d.variants);
            }
        }
        if !report.invalid_keys.is_empty() {
            println!("invalid_keys:");
            for d in &report.invalid_keys {
                println!("{} source={}", d.path, d.wanted.source);
            }
        }
        if !report.extraneous.is_empty() {
            println!("extraneous:");
            for p in &report.extraneous {
                println!("{}", p);
            }
        }
    }

    Ok(())
}
