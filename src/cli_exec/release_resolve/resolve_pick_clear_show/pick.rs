use super::super::*;

use super::pick_spec::{PickSpecifier, parse_pick_specifier};

pub(super) fn handle_resolve_pick(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    path: String,
    variant: Option<u32>,
    key: Option<String>,
    json: bool,
) -> Result<()> {
    let bundle = client.get_bundle(&bundle_id)?;
    let root = converge::model::ObjectId(bundle.root_manifest.clone());
    client.fetch_manifest_tree(&ws.store, &root)?;

    let variants = converge::resolve::superposition_variants(&ws.store, &root)?;
    let Some(vs) = variants.get(&path) else {
        anyhow::bail!("no superposition at path {}", path);
    };
    let vlen = vs.len();

    let decision = match parse_pick_specifier(variant, key, vlen)? {
        PickSpecifier::VariantIndex(idx) => converge::model::ResolutionDecision::Key(vs[idx].key()),
        PickSpecifier::KeyJson(key_json) => {
            let key: converge::model::VariantKey =
                serde_json::from_str(&key_json).context("parse --key")?;
            if !vs.iter().any(|v| v.key() == key) {
                anyhow::bail!("key not present at path {}", path);
            }
            converge::model::ResolutionDecision::Key(key)
        }
    };

    let mut r = ws.store.get_resolution(&bundle_id)?;
    if r.root_manifest != root {
        anyhow::bail!(
            "resolution root_manifest mismatch (resolution {}, bundle {})",
            r.root_manifest.as_str(),
            root.as_str()
        );
    }

    // Best-effort upgrade: convert index decisions to keys using current variants.
    if r.version == 1 {
        r.version = 2;
    }
    let existing = r.decisions.clone();
    for (p, d) in existing {
        if let converge::model::ResolutionDecision::Index(i) = d {
            let i = i as usize;
            if let Some(vs) = variants.get(&p)
                && i < vs.len()
            {
                r.decisions
                    .insert(p, converge::model::ResolutionDecision::Key(vs[i].key()));
            }
        }
    }

    r.decisions.insert(path.clone(), decision);
    ws.store.put_resolution(&r)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&r).context("serialize resolution")?
        );
    } else if let Some(v) = variant {
        println!("Picked variant #{} for {}", v, path);
    } else {
        println!("Picked key for {}", path);
    }

    Ok(())
}
