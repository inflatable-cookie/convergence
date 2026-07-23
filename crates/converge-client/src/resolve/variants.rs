use anyhow::Result;

use crate::model::{ManifestEntryKind, ObjectId, SuperpositionVariant};
use crate::store::LocalStore;

pub fn superposition_variants(
    store: &LocalStore,
    root: &ObjectId,
) -> Result<std::collections::BTreeMap<String, Vec<SuperpositionVariant>>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![(String::new(), root.clone())];

    while let Some((prefix, mid)) = stack.pop() {
        let manifest = store.get_manifest(&mid)?;
        for e in manifest.entries {
            let path = if prefix.is_empty() {
                e.name.clone()
            } else {
                format!("{}/{}", prefix, e.name)
            };

            match e.kind {
                ManifestEntryKind::Dir { manifest } => {
                    stack.push((path, manifest));
                }
                ManifestEntryKind::Superposition { variants } => {
                    out.insert(path, variants);
                }
                ManifestEntryKind::File { .. }
                | ManifestEntryKind::FileChunks { .. }
                | ManifestEntryKind::Symlink { .. } => {}
            }
        }
    }

    Ok(out)
}

pub fn superposition_variant_counts(
    store: &LocalStore,
    root: &ObjectId,
) -> Result<std::collections::BTreeMap<String, usize>> {
    let variants = superposition_variants(store, root)?;
    Ok(variants.into_iter().map(|(p, v)| (p, v.len())).collect())
}
