use std::collections::BTreeMap;

use anyhow::{Context, Result};

use converge_model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SuperpositionVariant,
    SuperpositionVariantKind,
};

use crate::storage::{ObjectKind, ObjectStore};

/// One publication's contribution to a bundle build (doc 17 §2).
pub struct MergeInput {
    /// Provenance source shown on superposition variants.
    pub lane: String,
    /// Root of the tree the publisher declared as base (`None` = empty).
    pub base: Option<ObjectId>,
    pub tree: ObjectId,
}

/// A publisher's opinion about one path.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Op {
    Set(ManifestEntryKind),
    Delete,
}

/// Base-aware fold (doc 17 §2-3): compute each input's delta against its
/// declared base, fold the opinions onto W. Unchanged paths express no
/// opinion; clean deletions remove paths; delete-vs-modify superposes with
/// a `Tombstone` variant. Deterministic: all maps are ordered.
pub fn merge_window(
    objects: &dyn ObjectStore,
    w_root: Option<&ObjectId>,
    inputs: &[MergeInput],
) -> Result<ObjectId> {
    let mut result: BTreeMap<String, ManifestEntryKind> = match w_root {
        Some(root) => flatten(objects, root)?,
        None => BTreeMap::new(),
    };

    // path -> ordered opinions
    let mut opinions: BTreeMap<String, Vec<(String, Op)>> = BTreeMap::new();
    let mut base_flats: Vec<BTreeMap<String, ManifestEntryKind>> = Vec::new();
    for input in inputs {
        let base = match &input.base {
            Some(root) => flatten(objects, root)?,
            None => BTreeMap::new(),
        };
        let tree = flatten(objects, &input.tree)?;

        for (path, kind) in &tree {
            match base.get(path) {
                Some(base_kind) if base_kind == kind => {} // no opinion
                _ => opinions
                    .entry(path.clone())
                    .or_default()
                    .push((input.lane.clone(), Op::Set(kind.clone()))),
            }
        }
        for path in base.keys() {
            if !tree.contains_key(path) {
                opinions
                    .entry(path.clone())
                    .or_default()
                    .push((input.lane.clone(), Op::Delete));
            }
        }
        base_flats.push(base);
    }

    // Supersession by base containment (doc 17 §2): drop a Set(k) when a
    // causally-newer input built on k AND the drop cannot lose content —
    // that input has its own explicit opinion at the path, or W carries k.
    let paths: Vec<String> = opinions.keys().cloned().collect();
    for path in paths {
        let ops = opinions.get(&path).expect("path present").clone();
        let retained: Vec<(String, Op)> = ops
            .iter()
            .filter(|(lane, op)| match op {
                Op::Delete => true,
                Op::Set(kind) => {
                    let superseded = base_flats.iter().zip(inputs).any(|(base, input)| {
                        input.lane != *lane
                            && base.get(&path) == Some(kind)
                            && (result.get(&path) == Some(kind)
                                || ops.iter().any(|(other_lane, other_op)| {
                                    *other_lane == input.lane && *other_op != Op::Set(kind.clone())
                                }))
                    });
                    !superseded
                }
            })
            .cloned()
            .collect();
        opinions.insert(path, retained);
    }
    opinions.retain(|_, ops| !ops.is_empty());

    for (path, ops) in opinions {
        // Distinct sets (dedup identical content, keep first source).
        let mut sets: Vec<(String, ManifestEntryKind)> = Vec::new();
        let mut deleters: Vec<String> = Vec::new();
        for (lane, op) in ops {
            match op {
                Op::Set(kind) => {
                    if !sets.iter().any(|(_, k)| *k == kind) {
                        sets.push((lane, kind));
                    }
                }
                Op::Delete => deleters.push(lane),
            }
        }

        // Drop sets that merely restate what W already holds.
        let current = result.get(&path);
        sets.retain(|(_, k)| current != Some(k));

        match (sets.len(), deleters.is_empty()) {
            (0, true) => {} // all opinions collapsed into W's value
            (0, false) => {
                result.remove(&path); // clean deletion
            }
            (1, true) => {
                let (_, kind) = sets.into_iter().next().expect("one set");
                result.insert(path, kind);
            }
            _ => {
                // True divergence (and/or delete-vs-modify).
                let mut variants: Vec<SuperpositionVariant> = sets
                    .into_iter()
                    .flat_map(|(lane, kind)| to_variants(lane, kind))
                    .collect();
                if let Some(lane) = deleters.into_iter().next() {
                    variants.push(SuperpositionVariant {
                        source: lane,
                        kind: SuperpositionVariantKind::Tombstone,
                    });
                }
                result.insert(path, ManifestEntryKind::Superposition { variants });
            }
        }
    }

    build_tree(objects, &result)
}

/// Leaf entries by path; directories recursed, superpositions kept as
/// leaves. Merkle short-circuit lives in the flatten cache of identical
/// subtree ids.
fn flatten(
    objects: &dyn ObjectStore,
    root: &ObjectId,
) -> Result<BTreeMap<String, ManifestEntryKind>> {
    let mut out = BTreeMap::new();
    flatten_into(objects, root, "", &mut out)?;
    Ok(out)
}

fn flatten_into(
    objects: &dyn ObjectStore,
    id: &ObjectId,
    prefix: &str,
    out: &mut BTreeMap<String, ManifestEntryKind>,
) -> Result<()> {
    let manifest = load_manifest(objects, id)?;
    for entry in manifest.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => {
                flatten_into(objects, &manifest, &path, out)?;
            }
            other => {
                out.insert(path, other);
            }
        }
    }
    Ok(())
}

/// Rebuild nested manifests from a flat path map. BTreeMap ordering makes
/// the result deterministic.
fn build_tree(
    objects: &dyn ObjectStore,
    entries: &BTreeMap<String, ManifestEntryKind>,
) -> Result<ObjectId> {
    let mut leaves: Vec<ManifestEntry> = Vec::new();
    let mut subdirs: BTreeMap<String, BTreeMap<String, ManifestEntryKind>> = BTreeMap::new();

    for (path, kind) in entries {
        match path.split_once('/') {
            None => leaves.push(ManifestEntry {
                name: path.clone(),
                kind: kind.clone(),
            }),
            Some((dir, rest)) => {
                subdirs
                    .entry(dir.to_string())
                    .or_default()
                    .insert(rest.to_string(), kind.clone());
            }
        }
    }

    for (dir, children) in subdirs {
        let manifest = build_tree(objects, &children)?;
        leaves.push(ManifestEntry {
            name: dir,
            kind: ManifestEntryKind::Dir { manifest },
        });
    }

    leaves.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = Manifest {
        version: 1,
        entries: leaves,
    };
    let bytes = serde_json::to_vec(&manifest).context("serialize merged manifest")?;
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
        // Nested superpositions flatten: inner variants keep their own
        // provenance.
        ManifestEntryKind::Superposition { variants } => return variants,
    };
    vec![SuperpositionVariant { source, kind }]
}
