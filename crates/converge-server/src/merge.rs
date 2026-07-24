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

/// Result of a fold: the merged root plus what the fold learned along
/// the way, so callers need no second walk (audit 2.2).
pub struct MergeOutcome {
    pub root: ObjectId,
    /// A superposition was written (or folded through from an input).
    /// W itself is superposition-free by construction — promote refuses a
    /// non-promotable bundle — so this is the complete answer.
    pub has_superpositions: bool,
}

/// Base-aware fold (doc 17 §2-3): compute each input's delta against its
/// declared base, fold the opinions onto W. Unchanged paths express no
/// opinion; clean deletions remove paths; delete-vs-modify superposes with
/// a `Tombstone` variant. Deterministic: all maps are ordered.
///
/// Cost is bounded by *changed* paths (doc 17 §2): input deltas come from
/// a diff that prunes on equal subtree ids, the values the fold needs from
/// W or another input's base are fetched by path walk, and the merged tree
/// rewrites only the manifests on changed paths — untouched subtrees keep
/// their existing ids.
pub fn merge_window(
    objects: &dyn ObjectStore,
    w_root: Option<&ObjectId>,
    inputs: &[MergeInput],
    strategy: &str,
) -> Result<ObjectId> {
    Ok(merge_window_outcome(objects, w_root, inputs, strategy)?.root)
}

pub fn merge_window_outcome(
    objects: &dyn ObjectStore,
    w_root: Option<&ObjectId>,
    inputs: &[MergeInput],
    strategy: &str,
) -> Result<MergeOutcome> {
    // path -> ordered opinions (input index, lane, op). Sparse: only
    // paths some input actually changed appear here.
    let mut opinions: BTreeMap<String, Vec<(usize, String, Op)>> = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        let mut delta = BTreeMap::new();
        diff_trees(
            objects,
            input.base.as_ref(),
            Some(&input.tree),
            "",
            &mut delta,
        )?;
        for (path, op) in delta {
            opinions
                .entry(path)
                .or_default()
                .push((index, input.lane.clone(), op));
        }
    }

    // Path walks are memoized by (root, path) across the whole fold
    // (batch 15.4). The supersession pass below asks every input's base
    // for every contested path, and a window's inputs overwhelmingly
    // declare the *same* base — without this the fold costs
    // paths × inputs walks, which the 100-publish benchmark measured as
    // 20k manifest reads. Objects are immutable, so the memo cannot go
    // stale mid-merge.
    let mut walked: BTreeMap<(ObjectId, String), Option<ManifestEntryKind>> = BTreeMap::new();

    // Values from W are needed only at contested paths, so they are read
    // by path walk rather than by flattening the whole tree.
    let mut w_at: BTreeMap<String, Option<ManifestEntryKind>> = BTreeMap::new();
    for path in opinions.keys() {
        let value = match w_root {
            Some(root) => lookup_path_memo(objects, &mut walked, root, path)?,
            None => None,
        };
        w_at.insert(path.clone(), value);
    }

    // Supersession by base containment (doc 17 §2): drop a Set(k) when a
    // causally-newer input built on k AND the drop cannot lose content —
    // that input has its own explicit opinion at the path, or W carries k.
    let paths: Vec<String> = opinions.keys().cloned().collect();
    for path in paths {
        let ops = opinions.get(&path).expect("path present").clone();
        let current = w_at.get(&path).and_then(|v| v.as_ref());
        let mut retained: Vec<(usize, String, Op)> = Vec::new();
        for (index, lane, op) in &ops {
            let keep = match op {
                Op::Delete => true,
                Op::Set(kind, _) => {
                    let mut superseded = false;
                    for (other, other_input) in inputs.iter().enumerate() {
                        if other == *index {
                            continue;
                        }
                        let other_base = match &other_input.base {
                            Some(root) => lookup_path_memo(objects, &mut walked, root, &path)?,
                            None => None,
                        };
                        if !base_contains(other_base.as_ref(), kind) {
                            continue;
                        }
                        let other_has_own_opinion = ops.iter().any(|(op_index, _, other_op)| {
                            *op_index == other && !matches!(other_op, Op::Set(k, _) if k == kind)
                        });
                        if current == Some(kind) || other_has_own_opinion {
                            superseded = true;
                            break;
                        }
                    }
                    !superseded
                }
            };
            if keep {
                retained.push((*index, lane.clone(), op.clone()));
            }
        }
        opinions.insert(path, retained);
    }
    opinions.retain(|_, ops| !ops.is_empty());

    // path -> new value (None = remove). Only changed paths appear.
    let mut changes: BTreeMap<String, Option<ManifestEntryKind>> = BTreeMap::new();
    let mut has_superpositions = false;

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
        let current = w_at.get(&path).cloned().flatten();
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
                changes.insert(path, None); // clean deletion
            }
            (1, true) => {
                let (_, kind) = sets.into_iter().next().expect("one set");
                has_superpositions |= matches!(kind, ManifestEntryKind::Superposition { .. });
                changes.insert(path, Some(kind));
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
                    changes.insert(path, Some(merged));
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
                has_superpositions = true;
                changes.insert(path, Some(ManifestEntryKind::Superposition { variants }));
            }
        }
    }

    // Rewrite only the manifests on changed paths; untouched subtrees
    // keep their existing ids, so nothing is re-hashed or re-stored for a
    // directory nobody edited.
    let root = apply_changes(objects, w_root, &changes)?;
    Ok(MergeOutcome {
        root,
        has_superpositions,
    })
}

/// Per-input delta with Merkle short-circuit (doc 17 §2): equal subtree
/// ids mean that whole subtree expresses no opinion and is never read.
fn diff_trees(
    objects: &dyn ObjectStore,
    base: Option<&ObjectId>,
    tree: Option<&ObjectId>,
    prefix: &str,
    out: &mut BTreeMap<String, Op>,
) -> Result<()> {
    if base == tree {
        return Ok(());
    }
    let base_entries = match base {
        Some(id) => entries_by_name(objects, id)?,
        None => BTreeMap::new(),
    };
    let tree_entries = match tree {
        Some(id) => entries_by_name(objects, id)?,
        None => BTreeMap::new(),
    };

    let names: std::collections::BTreeSet<&String> =
        base_entries.keys().chain(tree_entries.keys()).collect();
    for name in names {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let before = base_entries.get(name);
        let after = tree_entries.get(name);
        match (before, after) {
            (
                Some(ManifestEntryKind::Dir { manifest: b }),
                Some(ManifestEntryKind::Dir { manifest: t }),
            ) => {
                diff_trees(objects, Some(b), Some(t), &path, out)?;
            }
            // A directory replaced by a leaf (or vice versa): the leaves
            // under the directory read as deleted, the leaf as set.
            (Some(ManifestEntryKind::Dir { manifest: b }), after) => {
                diff_trees(objects, Some(b), None, &path, out)?;
                if let Some(kind) = after {
                    out.insert(path, Op::Set(kind.clone(), None));
                }
            }
            (before, Some(ManifestEntryKind::Dir { manifest: t })) => {
                if before.is_some() {
                    out.insert(path.clone(), Op::Delete);
                }
                diff_trees(objects, None, Some(t), &path, out)?;
            }
            (before, Some(kind)) => {
                if before != Some(kind) {
                    out.insert(path, Op::Set(kind.clone(), before.cloned()));
                }
            }
            (Some(_), None) => {
                out.insert(path, Op::Delete);
            }
            (None, None) => unreachable!("name came from one of the maps"),
        }
    }
    Ok(())
}

/// The value at `path`, walking only the manifests along it.
/// `lookup_path` with a fold-lifetime memo keyed by (root, path).
fn lookup_path_memo(
    objects: &dyn ObjectStore,
    walked: &mut BTreeMap<(ObjectId, String), Option<ManifestEntryKind>>,
    root: &ObjectId,
    path: &str,
) -> Result<Option<ManifestEntryKind>> {
    let key = (root.clone(), path.to_string());
    if let Some(hit) = walked.get(&key) {
        return Ok(hit.clone());
    }
    let value = lookup_path(objects, root, path)?;
    walked.insert(key, value.clone());
    Ok(value)
}

fn lookup_path(
    objects: &dyn ObjectStore,
    root: &ObjectId,
    path: &str,
) -> Result<Option<ManifestEntryKind>> {
    let mut current = root.clone();
    let mut segments = path.split('/').peekable();
    while let Some(segment) = segments.next() {
        let entries = entries_by_name(objects, &current)?;
        let Some(kind) = entries.get(segment) else {
            return Ok(None);
        };
        if segments.peek().is_none() {
            return Ok(Some(kind.clone()));
        }
        match kind {
            ManifestEntryKind::Dir { manifest } => current = manifest.clone(),
            _ => return Ok(None),
        }
    }
    Ok(None)
}

/// Apply path changes to `base`, rewriting only affected manifests.
fn apply_changes(
    objects: &dyn ObjectStore,
    base: Option<&ObjectId>,
    changes: &BTreeMap<String, Option<ManifestEntryKind>>,
) -> Result<ObjectId> {
    // Split each change into (first segment, rest) so a directory is
    // visited once with all of its pending edits.
    let mut here: BTreeMap<String, Option<ManifestEntryKind>> = BTreeMap::new();
    let mut nested: BTreeMap<String, BTreeMap<String, Option<ManifestEntryKind>>> = BTreeMap::new();
    for (path, value) in changes {
        match path.split_once('/') {
            None => {
                here.insert(path.clone(), value.clone());
            }
            Some((dir, rest)) => {
                nested
                    .entry(dir.to_string())
                    .or_default()
                    .insert(rest.to_string(), value.clone());
            }
        }
    }

    let mut entries = match base {
        Some(id) => entries_by_name(objects, id)?,
        None => BTreeMap::new(),
    };

    for (name, value) in here {
        match value {
            Some(kind) => {
                entries.insert(name, kind);
            }
            None => {
                entries.remove(&name);
            }
        }
    }

    for (dir, child_changes) in nested {
        let child_base = match entries.get(&dir) {
            Some(ManifestEntryKind::Dir { manifest }) => Some(manifest.clone()),
            // A leaf being replaced by a subtree starts from nothing.
            _ => None,
        };
        let rewritten = apply_changes(objects, child_base.as_ref(), &child_changes)?;
        if manifest_is_empty(objects, &rewritten)? {
            // A directory emptied by deletions disappears rather than
            // lingering as an empty entry.
            entries.remove(&dir);
        } else {
            entries.insert(
                dir,
                ManifestEntryKind::Dir {
                    manifest: rewritten,
                },
            );
        }
    }

    let mut out: Vec<ManifestEntry> = entries
        .into_iter()
        .map(|(name, kind)| ManifestEntry { name, kind })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    let manifest = Manifest {
        version: 1,
        entries: out,
    };
    objects.put(
        ObjectKind::Manifest,
        &converge_model::encoding::encode_manifest(&manifest),
    )
}

fn manifest_is_empty(objects: &dyn ObjectStore, id: &ObjectId) -> Result<bool> {
    Ok(load_manifest(objects, id)?.entries.is_empty())
}

fn entries_by_name(
    objects: &dyn ObjectStore,
    id: &ObjectId,
) -> Result<BTreeMap<String, ManifestEntryKind>> {
    Ok(load_manifest(objects, id)?
        .entries
        .into_iter()
        .map(|e| (e.name, e.kind))
        .collect())
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

fn load_manifest(objects: &dyn ObjectStore, id: &ObjectId) -> Result<Manifest> {
    let bytes = objects.get(ObjectKind::Manifest, id)?;
    converge_model::encoding::decode_manifest(&bytes)
        .with_context(|| format!("parse manifest {}", id.as_str()))
}

/// Does a declared base hold `kind` at a path — either as the value, or
/// as one of the variants of a superposition there (doc 17 §2)?
///
/// The variant case is what lets a resolution close the loop (batch
/// 16.1). A publisher who based on a superposed bundle and set a value
/// saw every variant and decided among them; re-superposing the losing
/// variants against that decision would make resolution impossible until
/// the window is promoted. Content is not at risk: the safety condition
/// below still requires the superseder to carry its own explicit opinion
/// at the path, or W to hold the value already.
fn base_contains(base: Option<&ManifestEntryKind>, kind: &ManifestEntryKind) -> bool {
    match base {
        Some(value) if value == kind => true,
        Some(ManifestEntryKind::Superposition { variants }) => variants
            .iter()
            .any(|variant| variant_matches(&variant.kind, kind)),
        _ => false,
    }
}

fn variant_matches(variant: &SuperpositionVariantKind, kind: &ManifestEntryKind) -> bool {
    match (variant, kind) {
        (
            SuperpositionVariantKind::File { blob, mode, size },
            ManifestEntryKind::File {
                blob: b,
                mode: m,
                size: s,
            },
        ) => blob == b && mode == m && size == s,
        (
            SuperpositionVariantKind::FileChunks { recipe, mode, size },
            ManifestEntryKind::FileChunks {
                recipe: r,
                mode: m,
                size: s,
            },
        ) => recipe == r && mode == m && size == s,
        (
            SuperpositionVariantKind::Dir { manifest },
            ManifestEntryKind::Dir { manifest: other },
        ) => manifest == other,
        (
            SuperpositionVariantKind::Symlink { target },
            ManifestEntryKind::Symlink { target: other },
        ) => target == other,
        // A tombstone is the absence of content, never a value someone set.
        _ => false,
    }
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
