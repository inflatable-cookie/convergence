use std::collections::BTreeMap;

use anyhow::{Context, Result};

use converge_model::{
    FileRecipe, Manifest, ManifestEntry, ManifestEntryKind, ObjectId, SuperpositionVariant,
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
    /// New value plus the value this publisher's own base held there
    /// (diff3 ancestor material).
    Set(ManifestEntryKind, Option<ManifestEntryKind>),
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
    strategy: &str,
) -> Result<ObjectId> {
    let mut result: BTreeMap<String, ManifestEntryKind> = match w_root {
        Some(root) => flatten(objects, root)?,
        None => BTreeMap::new(),
    };

    // path -> ordered opinions (input index, lane, op)
    let mut opinions: BTreeMap<String, Vec<(usize, String, Op)>> = BTreeMap::new();
    let mut base_flats: Vec<BTreeMap<String, ManifestEntryKind>> = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let base = match &input.base {
            Some(root) => flatten(objects, root)?,
            None => BTreeMap::new(),
        };
        let tree = flatten(objects, &input.tree)?;

        for (path, kind) in &tree {
            match base.get(path) {
                Some(base_kind) if base_kind == kind => {} // no opinion
                base_kind => opinions.entry(path.clone()).or_default().push((
                    index,
                    input.lane.clone(),
                    Op::Set(kind.clone(), base_kind.cloned()),
                )),
            }
        }
        for path in base.keys() {
            if !tree.contains_key(path) {
                opinions.entry(path.clone()).or_default().push((
                    index,
                    input.lane.clone(),
                    Op::Delete,
                ));
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
        let retained: Vec<(usize, String, Op)> = ops
            .iter()
            .filter(|(index, _, op)| match op {
                Op::Delete => true,
                Op::Set(kind, _) => {
                    let superseded = base_flats.iter().enumerate().any(|(other, base)| {
                        other != *index
                            && base.get(&path) == Some(kind)
                            && (result.get(&path) == Some(kind)
                                || ops.iter().any(|(op_index, _, other_op)| {
                                    *op_index == other
                                        && !matches!(other_op, Op::Set(k, _) if k == kind)
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
        let mut set_bases: Vec<Option<ManifestEntryKind>> = Vec::new();
        let mut deleters: Vec<String> = Vec::new();
        for (_, lane, op) in ops {
            match op {
                Op::Set(kind, base_kind) => {
                    if !sets.iter().any(|(_, k)| *k == kind) {
                        sets.push((lane, kind));
                        set_bases.push(base_kind);
                    }
                }
                Op::Delete => deleters.push(lane),
            }
        }

        // Drop sets that merely restate what W already holds — but only
        // when no deletion contests the path (doc 17 §2, audit H4):
        // against a Delete, restating W is an explicit keep opinion and
        // must survive into the superposition.
        let current = result.get(&path).cloned();
        let (sets, set_bases) = if deleters.is_empty() {
            let kept: Vec<(usize, (String, ManifestEntryKind))> = sets
                .into_iter()
                .enumerate()
                .filter(|(_, (_, k))| current.as_ref() != Some(k))
                .collect();
            let bases: Vec<Option<ManifestEntryKind>> =
                kept.iter().map(|(i, _)| set_bases[*i].clone()).collect();
            (kept.into_iter().map(|(_, s)| s).collect::<Vec<_>>(), bases)
        } else {
            (sets, set_bases)
        };

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
                // True divergence: dispatch to the gate's strategy first
                // (doc 17 §4); unresolved divergence superposes.
                // Diff3 ancestor (doc 17 §4): shared declared-base value if
                // the divergent opinions agree on one, else W's value.
                let ancestor = if !set_bases.is_empty()
                    && set_bases.iter().all(|b| b.is_some() && *b == set_bases[0])
                {
                    set_bases[0].clone()
                } else {
                    current.clone()
                };
                if strategy == "text-line-merge"
                    && deleters.is_empty()
                    && let Some(merged) = try_text_line_merge(objects, ancestor.as_ref(), &sets)?
                {
                    result.insert(path, merged);
                    continue;
                }
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

/// `text-line-merge` (doc 17 §4): diff3 the divergent variants against the
/// fold's current value, pairwise in input order. Clean merge -> a new
/// `File` entry; any overlapping hunk -> `None` (the caller superposes the
/// original variants — conflict markers are never written). Non-text
/// content -> `None` (per-path fallback to whole-file behavior).
fn try_text_line_merge(
    objects: &dyn ObjectStore,
    base: Option<&ManifestEntryKind>,
    sets: &[(String, ManifestEntryKind)],
) -> Result<Option<ManifestEntryKind>> {
    let base_text = match base {
        Some(kind) => match file_text(objects, kind)? {
            Some(text) => text,
            None => return Ok(None),
        },
        None => String::new(),
    };

    let mut variant_texts = Vec::new();
    let mut modes = Vec::new();
    for (_, kind) in sets {
        match file_text(objects, kind)? {
            Some(text) => variant_texts.push(text),
            None => return Ok(None),
        }
        modes.push(match kind {
            ManifestEntryKind::File { mode, .. } | ManifestEntryKind::FileChunks { mode, .. } => {
                *mode
            }
            _ => return Ok(None),
        });
    }

    let mut merged = variant_texts[0].clone();
    for variant in &variant_texts[1..] {
        match diffy::merge(&base_text, &merged, variant) {
            Ok(clean) => merged = clean,
            // Overlapping hunks: conflicts stay data, never markers.
            Err(_) => return Ok(None),
        }
    }

    let mode = if modes.iter().all(|m| *m == modes[0]) {
        modes[0]
    } else {
        match base {
            Some(ManifestEntryKind::File { mode, .. })
            | Some(ManifestEntryKind::FileChunks { mode, .. }) => *mode,
            _ => 0o644,
        }
    };
    let bytes = merged.into_bytes();
    let blob = objects.put(ObjectKind::Blob, &bytes)?;
    Ok(Some(ManifestEntryKind::File {
        blob,
        mode,
        size: bytes.len() as u64,
    }))
}

/// Load file-like content and admit it as text: File or FileChunks, no NUL
/// byte in the first 8 KiB, valid UTF-8.
fn file_text(objects: &dyn ObjectStore, kind: &ManifestEntryKind) -> Result<Option<String>> {
    let bytes = match kind {
        ManifestEntryKind::File { blob, .. } => objects.get(ObjectKind::Blob, blob)?,
        ManifestEntryKind::FileChunks { recipe, .. } => {
            let recipe: FileRecipe =
                converge_model::encoding::decode_recipe(&objects.get(ObjectKind::Recipe, recipe)?)?;
            let mut out = Vec::with_capacity(recipe.size as usize);
            for chunk in &recipe.chunks {
                out.extend_from_slice(&objects.get(ObjectKind::Blob, &chunk.blob)?);
            }
            out
        }
        _ => return Ok(None),
    };
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Ok(None);
    }
    Ok(String::from_utf8(bytes).ok())
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
    let bytes = converge_model::encoding::encode_manifest(&manifest);
    objects.put(ObjectKind::Manifest, &bytes)
}

fn load_manifest(objects: &dyn ObjectStore, id: &ObjectId) -> Result<Manifest> {
    let bytes = objects.get(ObjectKind::Manifest, id)?;
    converge_model::encoding::decode_manifest(&bytes)
        .with_context(|| format!("parse manifest {}", id.as_str()))
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
