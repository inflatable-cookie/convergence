use std::collections::BTreeMap;

use anyhow::{Context, Result};

use converge_model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SuperpositionVariant,
    SuperpositionVariantKind,
};

use crate::storage::{ObjectKind, ObjectStore};

/// Deterministic Merkle merge of input manifests (arch 14 §5).
///
/// - identical entries pass through; identical subtree hashes short-circuit
/// - entries present in only some inputs pass through (union semantics; the
///   slice has no base manifests yet, so absence is not a deletion signal)
/// - all-dir divergence recurses
/// - any other divergence becomes a `Superposition` with one variant per
///   distinct content, provenance = the first source carrying that content
pub fn merge_manifests(
    objects: &dyn ObjectStore,
    inputs: &[(String, ObjectId)],
) -> Result<ObjectId> {
    assert!(!inputs.is_empty(), "merge requires at least one input");
    if inputs.iter().all(|(_, id)| *id == inputs[0].1) {
        return Ok(inputs[0].1.clone());
    }

    // name -> ordered (source, kind)
    let mut by_name: BTreeMap<String, Vec<(String, ManifestEntryKind)>> = BTreeMap::new();
    for (source, manifest_id) in inputs {
        let manifest = load_manifest(objects, manifest_id)?;
        for entry in manifest.entries {
            by_name
                .entry(entry.name)
                .or_default()
                .push((source.clone(), entry.kind));
        }
    }

    let mut entries = Vec::new();
    for (name, variants) in by_name {
        let mut distinct: Vec<(String, ManifestEntryKind)> = Vec::new();
        for (source, kind) in variants {
            if !distinct.iter().any(|(_, k)| *k == kind) {
                distinct.push((source, kind));
            }
        }

        let kind = if distinct.len() == 1 {
            distinct.into_iter().next().expect("one variant").1
        } else if distinct
            .iter()
            .all(|(_, k)| matches!(k, ManifestEntryKind::Dir { .. }))
        {
            let sub_inputs: Vec<(String, ObjectId)> = distinct
                .into_iter()
                .map(|(source, kind)| match kind {
                    ManifestEntryKind::Dir { manifest } => (source, manifest),
                    _ => unreachable!("all dirs"),
                })
                .collect();
            ManifestEntryKind::Dir {
                manifest: merge_manifests(objects, &sub_inputs)?,
            }
        } else {
            ManifestEntryKind::Superposition {
                variants: distinct
                    .into_iter()
                    .flat_map(|(source, kind)| to_variants(source, kind))
                    .collect(),
            }
        };

        entries.push(ManifestEntry { name, kind });
    }

    let merged = Manifest {
        version: 1,
        entries,
    };
    let bytes = serde_json::to_vec(&merged).context("serialize merged manifest")?;
    objects.put(ObjectKind::Manifest, &bytes)
}

fn load_manifest(objects: &dyn ObjectStore, id: &ObjectId) -> Result<Manifest> {
    let bytes = objects.get(ObjectKind::Manifest, id)?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse manifest {}", id.as_str()))
}

fn to_variants(source: String, kind: ManifestEntryKind) -> Vec<SuperpositionVariant> {
    let kind = match kind {
        ManifestEntryKind::File { blob, mode, size } => {
            SuperpositionVariantKind::File { blob, mode, size }
        }
        ManifestEntryKind::FileChunks { recipe, mode, size } => {
            SuperpositionVariantKind::FileChunks { recipe, mode, size }
        }
        ManifestEntryKind::Dir { manifest } => SuperpositionVariantKind::Dir { manifest },
        ManifestEntryKind::Symlink { target } => SuperpositionVariantKind::Symlink { target },
        // Nested superpositions flatten: each inner variant becomes an outer
        // variant carrying its own provenance.
        ManifestEntryKind::Superposition { variants } => {
            return variants;
        }
    };
    vec![SuperpositionVariant { source, kind }]
}
