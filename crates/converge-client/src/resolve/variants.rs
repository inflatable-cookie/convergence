use anyhow::Result;

use crate::model::{
    ManifestEntryKind, ObjectId, ResolutionDecision, SuperpositionVariant, SuperpositionVariantKind,
};
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

/// Superpositions that a resolution must decide, walked the way
/// `apply_resolution` rewrites the tree (batch 13.4, audit C1): when a
/// decision selects a `Dir` variant, the chosen subtree is entered under
/// the same path, so nested superpositions inside it are required too.
/// Without a valid decision for a superposition the walk cannot know
/// which subtree applies, so it stops there — that path reports as
/// missing/invalid first, and its nested requirements surface once it is
/// decided.
pub fn required_superpositions(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
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
                    let chosen = decisions
                        .get(&path)
                        .and_then(|decision| variant_for(decision, &variants));
                    if let Some(SuperpositionVariantKind::Dir { manifest }) = chosen {
                        stack.push((path.clone(), manifest.clone()));
                    }
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

/// The variant a decision selects, or `None` when it resolves to nothing
/// (out of range / unknown key — reported by validation itself).
pub fn variant_for<'v>(
    decision: &ResolutionDecision,
    variants: &'v [SuperpositionVariant],
) -> Option<&'v SuperpositionVariantKind> {
    let index = match decision {
        ResolutionDecision::Index(i) => *i as usize,
        ResolutionDecision::Key(key) => variants.iter().position(|v| &v.key() == key)?,
    };
    variants.get(index).map(|v| &v.kind)
}

pub fn superposition_variant_counts(
    store: &LocalStore,
    root: &ObjectId,
) -> Result<std::collections::BTreeMap<String, usize>> {
    let variants = superposition_variants(store, root)?;
    Ok(variants.into_iter().map(|(p, v)| (p, v.len())).collect())
}
