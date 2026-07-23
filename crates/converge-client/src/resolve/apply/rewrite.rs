use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, ResolutionDecision,
    SuperpositionVariantKind,
};
use crate::store::LocalStore;

use super::decisions::decision_to_index;

pub(super) fn rewrite_manifest(
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
                let rewritten = rewrite_manifest(store, &manifest, &path, decisions, memo)?;
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
                    SuperpositionVariantKind::FileChunks { recipe, mode, size } => {
                        ManifestEntryKind::FileChunks {
                            recipe: recipe.clone(),
                            mode: *mode,
                            size: *size,
                        }
                    }
                    SuperpositionVariantKind::Dir { manifest } => {
                        let rewritten = rewrite_manifest(store, manifest, &path, decisions, memo)?;
                        ManifestEntryKind::Dir {
                            manifest: rewritten,
                        }
                    }
                    SuperpositionVariantKind::Symlink { target } => ManifestEntryKind::Symlink {
                        target: target.clone(),
                    },
                    SuperpositionVariantKind::Tombstone => {
                        // Drop entry entirely.
                        continue;
                    }
                }
            }
            ManifestEntryKind::File { blob, mode, size } => {
                ManifestEntryKind::File { blob, mode, size }
            }
            ManifestEntryKind::FileChunks { recipe, mode, size } => {
                ManifestEntryKind::FileChunks { recipe, mode, size }
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
