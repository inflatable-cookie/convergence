use anyhow::Result;

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariant, SuperpositionVariantKind};
use crate::store::LocalStore;

pub(super) fn collect_reachable_objects(
    store: &LocalStore,
    root: &ObjectId,
    blobs: &mut std::collections::HashSet<String>,
    manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
) -> Result<()> {
    let mut stack = vec![root.clone()];
    while let Some(mid) = stack.pop() {
        if !manifests.insert(mid.as_str().to_string()) {
            continue;
        }
        let m = store.get_manifest(&mid)?;
        for e in m.entries {
            match e.kind {
                ManifestEntryKind::Dir { manifest } => {
                    stack.push(manifest);
                }
                ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                ManifestEntryKind::FileChunks { recipe, .. } => {
                    let rid = recipe.as_str().to_string();
                    if recipes.insert(rid) {
                        let r = store.get_recipe(&recipe)?;
                        for c in r.chunks {
                            blobs.insert(c.blob.as_str().to_string());
                        }
                    }
                }
                ManifestEntryKind::Symlink { .. } => {}
                ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        collect_variant_objects(store, &v, blobs, manifests, recipes, &mut stack)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn collect_variant_objects(
    store: &LocalStore,
    v: &SuperpositionVariant,
    blobs: &mut std::collections::HashSet<String>,
    _manifests: &mut std::collections::HashSet<String>,
    recipes: &mut std::collections::HashSet<String>,
    stack: &mut Vec<ObjectId>,
) -> Result<()> {
    match &v.kind {
        SuperpositionVariantKind::File { blob, .. } => {
            blobs.insert(blob.as_str().to_string());
        }
        SuperpositionVariantKind::FileChunks { recipe, .. } => {
            let rid = recipe.as_str().to_string();
            if recipes.insert(rid) {
                let r = store.get_recipe(recipe)?;
                for c in r.chunks {
                    blobs.insert(c.blob.as_str().to_string());
                }
            }
        }
        SuperpositionVariantKind::Dir { manifest } => {
            stack.push(manifest.clone());
        }
        SuperpositionVariantKind::Symlink { .. } => {}
        SuperpositionVariantKind::Tombstone => {}
    }
    Ok(())
}
