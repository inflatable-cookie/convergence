use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ObjectId, ResolutionDecision, SuperpositionVariant,
    SuperpositionVariantKind, VariantKey,
};
use crate::store::LocalStore;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResolutionValidation {
    pub ok: bool,
    pub missing: Vec<String>,
    pub extraneous: Vec<String>,
    pub out_of_range: Vec<OutOfRangeDecision>,
    pub invalid_keys: Vec<InvalidKeyDecision>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OutOfRangeDecision {
    pub path: String,
    pub index: u32,
    pub variants: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InvalidKeyDecision {
    pub path: String,
    pub wanted: VariantKey,
    pub available: Vec<VariantKey>,
}

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

pub fn validate_resolution(
    store: &LocalStore,
    root: &ObjectId,
    decisions: &std::collections::BTreeMap<String, ResolutionDecision>,
) -> Result<ResolutionValidation> {
    let variants = superposition_variants(store, root)?;

    let mut missing = Vec::new();
    for p in variants.keys() {
        if !decisions.contains_key(p) {
            missing.push(p.clone());
        }
    }

    let mut extraneous = Vec::new();
    for p in decisions.keys() {
        if !variants.contains_key(p) {
            extraneous.push(p.clone());
        }
    }

    let mut out_of_range = Vec::new();
    let mut invalid_keys = Vec::new();
    for (path, decision) in decisions {
        let Some(vs) = variants.get(path) else {
            continue;
        };

        match decision {
            ResolutionDecision::Index(i) => {
                let idx = *i as usize;
                if idx >= vs.len() {
                    out_of_range.push(OutOfRangeDecision {
                        path: path.clone(),
                        index: *i,
                        variants: vs.len(),
                    });
                }
            }
            ResolutionDecision::Key(k) => {
                if !vs.iter().any(|v| &v.key() == k) {
                    invalid_keys.push(InvalidKeyDecision {
                        path: path.clone(),
                        wanted: k.clone(),
                        available: vs.iter().map(|v| v.key()).collect(),
                    });
                }
            }
        }
    }

    missing.sort();
    extraneous.sort();
    out_of_range.sort_by(|a, b| a.path.cmp(&b.path));
    invalid_keys.sort_by(|a, b| a.path.cmp(&b.path));

    let ok = missing.is_empty() && out_of_range.is_empty() && invalid_keys.is_empty();
    Ok(ResolutionValidation {
        ok,
        missing,
        extraneous,
        out_of_range,
        invalid_keys,
    })
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
    // Validate up front so users get a single actionable error.
    let report = validate_resolution(store, root, decisions)?;
    if !report.ok {
        fn head(xs: &[String]) -> String {
            const LIMIT: usize = 10;
            if xs.len() <= LIMIT {
                xs.join(", ")
            } else {
                format!("{} ... (+{})", xs[..LIMIT].join(", "), xs.len() - LIMIT)
            }
        }

        let mut parts = Vec::new();
        if !report.missing.is_empty() {
            parts.push(format!("missing=[{}]", head(&report.missing)));
        }
        if !report.extraneous.is_empty() {
            parts.push(format!("extraneous=[{}]", head(&report.extraneous)));
        }
        if !report.out_of_range.is_empty() {
            parts.push(format!("out_of_range={}", report.out_of_range.len()));
        }
        if !report.invalid_keys.is_empty() {
            parts.push(format!("invalid_keys={}", report.invalid_keys.len()));
        }
        anyhow::bail!("resolution invalid: {}", parts.join(" "));
    }

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
            ResolutionDecision::Key(key) => match find_variant_index_by_key(variants, key) {
                Some(i) => Ok(i),
                None => {
                    let mut available = Vec::new();
                    for v in variants {
                        let kj = serde_json::to_string(&v.key())
                            .unwrap_or_else(|_| "<unserializable-key>".to_string());
                        available.push(kj);
                    }
                    anyhow::bail!(
                            "resolution variant_key not found for {} (wanted source={}); available keys: {}",
                            path,
                            key.source,
                            available.join(", ")
                        );
                }
            },
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
