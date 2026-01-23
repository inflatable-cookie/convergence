use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, ResolutionDecision, SuperpositionVariant,
    SuperpositionVariantKind, VariantKey,
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
                ManifestEntryKind::File { .. } | ManifestEntryKind::Symlink { .. } => {}
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

pub fn apply_resolution(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
) -> Result<ObjectId> {
    fn find_variant_index_by_key(
        variants: &[SuperpositionVariant],
        key: &VariantKey,
    ) -> Option<usize> {
        variants.iter().position(|v| &v.key() == key)
    }

    fn decision_to_index(
        path: &str,
        decision: &ResolutionDecision,
        variants: &[SuperpositionVariant],
    ) -> Result<usize> {
        match decision {
            ResolutionDecision::Index(idx) => {
                let idx = *idx as usize;
                if idx >= variants.len() {
                    anyhow::bail!(
                        "resolution decision out of range for {} (idx {}, variants {})",
                        path,
                        idx,
                        variants.len()
                    );
                }
                Ok(idx)
            }
            ResolutionDecision::Key(key) => {
                find_variant_index_by_key(variants, key).with_context(|| {
                    format!(
                        "resolution variant_key not found for {} (source {})",
                        path, key.source
                    )
                })
            }
        }
    }

    // Rewrites the manifest graph, swapping superpositions for chosen variants.
    fn rewrite(
        store: &LocalStore,
        id: &ObjectId,
        prefix: &str,
        decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
        memo: &mut HashMap<String, ObjectId>,
    ) -> Result<ObjectId> {
        // Memoize by (prefix, manifest_id). Decisions are path-based, so identical manifest ids
        // reused at different paths must not share rewritten output.
        let memo_key = format!("{}::{}", prefix, id.as_str());
        if let Some(out) = memo.get(&memo_key) {
            return Ok(out.clone());
        }

        let manifest = store.get_manifest(id)?;
        let mut out_entries = Vec::with_capacity(manifest.entries.len());

        for e in manifest.entries {
            let path = if prefix.is_empty() {
                e.name.clone()
            } else {
                format!("{}/{}", prefix, e.name)
            };

            let kind = match e.kind {
                ManifestEntryKind::Dir { manifest } => {
                    let rewritten = rewrite(store, &manifest, &path, decisions, memo)?;
                    ManifestEntryKind::Dir {
                        manifest: rewritten,
                    }
                }
                ManifestEntryKind::Superposition { variants } => {
                    let decision = decisions
                        .get(&path)
                        .with_context(|| format!("no resolution decision for {}", path))?;
                    let idx = decision_to_index(&path, decision, &variants)?;

                    let v = &variants[idx];
                    match &v.kind {
                        SuperpositionVariantKind::File { blob, mode, size } => {
                            ManifestEntryKind::File {
                                blob: blob.clone(),
                                mode: *mode,
                                size: *size,
                            }
                        }
                        SuperpositionVariantKind::Dir { manifest } => {
                            let rewritten = rewrite(store, manifest, &path, decisions, memo)?;
                            ManifestEntryKind::Dir {
                                manifest: rewritten,
                            }
                        }
                        SuperpositionVariantKind::Symlink { target } => {
                            ManifestEntryKind::Symlink {
                                target: target.clone(),
                            }
                        }
                        SuperpositionVariantKind::Tombstone => {
                            // Drop entry entirely.
                            continue;
                        }
                    }
                }
                ManifestEntryKind::File { blob, mode, size } => {
                    ManifestEntryKind::File { blob, mode, size }
                }
                ManifestEntryKind::Symlink { target } => ManifestEntryKind::Symlink { target },
            };

            out_entries.push(ManifestEntry { name: e.name, kind });
        }

        // Deterministic order.
        out_entries.sort_by(|a, b| a.name.cmp(&b.name));

        let out_manifest = Manifest {
            version: 1,
            entries: out_entries,
        };
        let out_id = store.put_manifest(&out_manifest)?;
        memo.insert(memo_key, out_id.clone());
        Ok(out_id)
    }

    let mut memo = HashMap::new();
    rewrite(store, root, "", decisions, &mut memo)
}
