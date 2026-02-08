//! Manifest/snap object graph traversal, validation, and merge helpers.

use super::*;

pub(super) fn validate_manifest_entry_refs(
    state: &AppState,
    repo_id: &str,
    kind: &converge::model::ManifestEntryKind,
    allow_missing_blobs: bool,
) -> Result<(), Response> {
    match kind {
        converge::model::ManifestEntryKind::File { blob, .. } => {
            validate_object_id(blob.as_str()).map_err(bad_request)?;
            if !allow_missing_blobs {
                let p = repo_data_dir(state, repo_id)
                    .join("objects/blobs")
                    .join(blob.as_str());
                if !p.exists() {
                    return Err(bad_request(anyhow::anyhow!(
                        "missing referenced blob {}",
                        blob.as_str()
                    )));
                }
            }
        }
        converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
            validate_object_id(recipe.as_str()).map_err(bad_request)?;
            let p = repo_data_dir(state, repo_id)
                .join("objects/recipes")
                .join(format!("{}.json", recipe.as_str()));
            if !p.exists() {
                return Err(bad_request(anyhow::anyhow!(
                    "missing referenced recipe {}",
                    recipe.as_str()
                )));
            }
        }
        converge::model::ManifestEntryKind::Dir { manifest } => {
            validate_object_id(manifest.as_str()).map_err(bad_request)?;
            let p = repo_data_dir(state, repo_id)
                .join("objects/manifests")
                .join(format!("{}.json", manifest.as_str()));
            if !p.exists() {
                return Err(bad_request(anyhow::anyhow!(
                    "missing referenced manifest {}",
                    manifest.as_str()
                )));
            }
        }
        converge::model::ManifestEntryKind::Symlink { .. } => {}
        converge::model::ManifestEntryKind::Superposition { variants } => {
            for v in variants {
                match &v.kind {
                    converge::model::SuperpositionVariantKind::File { blob, .. } => {
                        validate_object_id(blob.as_str()).map_err(bad_request)?;
                        if !allow_missing_blobs {
                            let p = repo_data_dir(state, repo_id)
                                .join("objects/blobs")
                                .join(blob.as_str());
                            if !p.exists() {
                                return Err(bad_request(anyhow::anyhow!(
                                    "missing referenced blob {}",
                                    blob.as_str()
                                )));
                            }
                        }
                    }
                    converge::model::SuperpositionVariantKind::FileChunks { recipe, .. } => {
                        validate_object_id(recipe.as_str()).map_err(bad_request)?;
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/recipes")
                            .join(format!("{}.json", recipe.as_str()));
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced recipe {}",
                                recipe.as_str()
                            )));
                        }
                    }
                    converge::model::SuperpositionVariantKind::Dir { manifest } => {
                        validate_object_id(manifest.as_str()).map_err(bad_request)?;
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/manifests")
                            .join(format!("{}.json", manifest.as_str()));
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced manifest {}",
                                manifest.as_str()
                            )));
                        }
                    }
                    converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                    converge::model::SuperpositionVariantKind::Tombstone => {}
                }
            }
        }
    }
    Ok(())
}

pub(super) fn read_recipe(
    state: &AppState,
    repo_id: &str,
    recipe_id: &str,
) -> Result<converge::model::FileRecipe, Response> {
    validate_object_id(recipe_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/recipes")
        .join(format!("{}.json", recipe_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown recipe")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != recipe_id {
        return Err(internal_error(anyhow::anyhow!(
            "recipe integrity check failed"
        )));
    }
    let recipe: converge::model::FileRecipe =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(recipe)
}

pub(super) fn collect_objects_from_manifest_tree(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    blobs: &mut HashSet<String>,
    manifests: &mut HashSet<String>,
    recipes: &mut HashSet<String>,
) -> Result<(), Response> {
    fn visit_recipe(
        state: &AppState,
        repo_id: &str,
        recipe_id: &str,
        blobs: &mut HashSet<String>,
        recipes: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited.insert(recipe_id.to_string()) {
            return Ok(());
        }
        recipes.insert(recipe_id.to_string());
        let recipe = read_recipe(state, repo_id, recipe_id)?;
        for c in recipe.chunks {
            blobs.insert(c.blob.as_str().to_string());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_manifest(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        blobs: &mut HashSet<String>,
        manifests: &mut HashSet<String>,
        recipes: &mut HashSet<String>,
        visited_manifests: &mut HashSet<String>,
        visited_recipes: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited_manifests.insert(manifest_id.to_string()) {
            return Ok(());
        }
        manifests.insert(manifest_id.to_string());

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                    visit_recipe(
                        state,
                        repo_id,
                        recipe.as_str(),
                        blobs,
                        recipes,
                        visited_recipes,
                    )?;
                }
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    visit_manifest(
                        state,
                        repo_id,
                        manifest.as_str(),
                        blobs,
                        manifests,
                        recipes,
                        visited_manifests,
                        visited_recipes,
                    )?;
                }
                converge::model::ManifestEntryKind::Symlink { .. } => {}
                converge::model::ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        match v.kind {
                            converge::model::SuperpositionVariantKind::File { blob, .. } => {
                                blobs.insert(blob.as_str().to_string());
                            }
                            converge::model::SuperpositionVariantKind::FileChunks {
                                recipe,
                                ..
                            } => {
                                visit_recipe(
                                    state,
                                    repo_id,
                                    recipe.as_str(),
                                    blobs,
                                    recipes,
                                    visited_recipes,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Dir { manifest } => {
                                visit_manifest(
                                    state,
                                    repo_id,
                                    manifest.as_str(),
                                    blobs,
                                    manifests,
                                    recipes,
                                    visited_manifests,
                                    visited_recipes,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                            converge::model::SuperpositionVariantKind::Tombstone => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }

    visit_manifest(
        state,
        repo_id,
        root_manifest_id,
        blobs,
        manifests,
        recipes,
        &mut HashSet::new(),
        &mut HashSet::new(),
    )
}

pub(super) fn validate_manifest_tree_availability(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
    require_blobs: bool,
) -> Result<(), Response> {
    fn visit_manifest(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        require_blobs: bool,
        visited: &mut HashSet<String>,
    ) -> Result<(), Response> {
        if !visited.insert(manifest_id.to_string()) {
            return Ok(());
        }

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::File { blob, .. } => {
                    validate_object_id(blob.as_str()).map_err(bad_request)?;
                    if require_blobs {
                        let p = repo_data_dir(state, repo_id)
                            .join("objects/blobs")
                            .join(blob.as_str());
                        if !p.exists() {
                            return Err(bad_request(anyhow::anyhow!(
                                "missing referenced blob {}",
                                blob.as_str()
                            )));
                        }
                    }
                }
                converge::model::ManifestEntryKind::FileChunks { recipe, .. } => {
                    let recipe = read_recipe(state, repo_id, recipe.as_str())?;
                    for c in recipe.chunks {
                        validate_object_id(c.blob.as_str()).map_err(bad_request)?;
                        if require_blobs {
                            let p = repo_data_dir(state, repo_id)
                                .join("objects/blobs")
                                .join(c.blob.as_str());
                            if !p.exists() {
                                return Err(bad_request(anyhow::anyhow!(
                                    "missing referenced blob {}",
                                    c.blob.as_str()
                                )));
                            }
                        }
                    }
                }
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    visit_manifest(state, repo_id, manifest.as_str(), require_blobs, visited)?;
                }
                converge::model::ManifestEntryKind::Symlink { .. } => {}
                converge::model::ManifestEntryKind::Superposition { variants } => {
                    for v in variants {
                        match v.kind {
                            converge::model::SuperpositionVariantKind::File { blob, .. } => {
                                validate_object_id(blob.as_str()).map_err(bad_request)?;
                                if require_blobs {
                                    let p = repo_data_dir(state, repo_id)
                                        .join("objects/blobs")
                                        .join(blob.as_str());
                                    if !p.exists() {
                                        return Err(bad_request(anyhow::anyhow!(
                                            "missing referenced blob {}",
                                            blob.as_str()
                                        )));
                                    }
                                }
                            }
                            converge::model::SuperpositionVariantKind::FileChunks {
                                recipe,
                                ..
                            } => {
                                let recipe = read_recipe(state, repo_id, recipe.as_str())?;
                                for c in recipe.chunks {
                                    validate_object_id(c.blob.as_str()).map_err(bad_request)?;
                                    if require_blobs {
                                        let p = repo_data_dir(state, repo_id)
                                            .join("objects/blobs")
                                            .join(c.blob.as_str());
                                        if !p.exists() {
                                            return Err(bad_request(anyhow::anyhow!(
                                                "missing referenced blob {}",
                                                c.blob.as_str()
                                            )));
                                        }
                                    }
                                }
                            }
                            converge::model::SuperpositionVariantKind::Dir { manifest } => {
                                visit_manifest(
                                    state,
                                    repo_id,
                                    manifest.as_str(),
                                    require_blobs,
                                    visited,
                                )?;
                            }
                            converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                            converge::model::SuperpositionVariantKind::Tombstone => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }

    visit_manifest(
        state,
        repo_id,
        root_manifest_id,
        require_blobs,
        &mut HashSet::new(),
    )
}

pub(super) fn read_snap(
    state: &AppState,
    repo_id: &str,
    snap_id: &str,
) -> Result<converge::model::SnapRecord, Response> {
    validate_object_id(snap_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/snaps")
        .join(format!("{}.json", snap_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown snap")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let snap: converge::model::SnapRecord =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(snap)
}

pub(super) fn read_manifest(
    state: &AppState,
    repo_id: &str,
    manifest_id: &str,
) -> Result<converge::model::Manifest, Response> {
    validate_object_id(manifest_id).map_err(bad_request)?;
    let path = repo_data_dir(state, repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", manifest_id));
    if !path.exists() {
        return Err(bad_request(anyhow::anyhow!("unknown manifest")));
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read {}", path.display()))
        .map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != manifest_id {
        return Err(internal_error(anyhow::anyhow!(
            "manifest integrity check failed"
        )));
    }
    let manifest: converge::model::Manifest =
        serde_json::from_slice(&bytes).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    Ok(manifest)
}

pub(super) fn store_manifest(
    state: &AppState,
    repo_id: &str,
    manifest: &converge::model::Manifest,
) -> Result<String, Response> {
    let bytes = serde_json::to_vec(manifest).map_err(|e| internal_error(anyhow::anyhow!(e)))?;
    let id = blake3::hash(&bytes).to_hex().to_string();
    let path = repo_data_dir(state, repo_id)
        .join("objects/manifests")
        .join(format!("{}.json", id));
    write_if_absent(&path, &bytes).map_err(internal_error)?;
    Ok(id)
}

pub(super) fn coalesce_root_manifest(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    // inputs: (publication_id, root_manifest_id)
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.0.cmp(&b.0));
    merge_dir_manifests(state, repo_id, &inputs)
}

pub(super) fn manifest_has_superpositions(
    state: &AppState,
    repo_id: &str,
    root_manifest_id: &str,
) -> Result<bool, Response> {
    fn inner(
        state: &AppState,
        repo_id: &str,
        manifest_id: &str,
        visited: &mut HashSet<String>,
    ) -> Result<bool, Response> {
        if !visited.insert(manifest_id.to_string()) {
            return Ok(false);
        }

        let manifest = read_manifest(state, repo_id, manifest_id)?;
        for e in manifest.entries {
            match e.kind {
                converge::model::ManifestEntryKind::Superposition { .. } => return Ok(true),
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    if inner(state, repo_id, manifest.as_str(), visited)? {
                        return Ok(true);
                    }
                }
                converge::model::ManifestEntryKind::File { .. } => {}
                converge::model::ManifestEntryKind::FileChunks { .. } => {}
                converge::model::ManifestEntryKind::Symlink { .. } => {}
            }
        }
        Ok(false)
    }

    inner(state, repo_id, root_manifest_id, &mut HashSet::new())
}

pub(super) fn compute_promotability(
    gate: &GateDef,
    has_superpositions: bool,
    approval_count: usize,
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if has_superpositions && !gate.allow_superpositions {
        reasons.push("superpositions_present".to_string());
    }
    if approval_count < gate.required_approvals as usize {
        reasons.push("approvals_missing".to_string());
    }
    (reasons.is_empty(), reasons)
}

pub(super) fn merge_dir_manifests(
    state: &AppState,
    repo_id: &str,
    inputs: &[(String, String)],
) -> Result<String, Response> {
    use std::collections::{BTreeMap, BTreeSet};

    // Load each input directory manifest.
    let mut input_maps: Vec<(String, BTreeMap<String, converge::model::ManifestEntryKind>)> =
        Vec::new();
    for (pub_id, mid) in inputs {
        let m = read_manifest(state, repo_id, mid)?;
        let mut map = BTreeMap::new();
        for e in m.entries {
            map.insert(e.name, e.kind);
        }
        input_maps.push((pub_id.clone(), map));
    }

    // Union of entry names.
    let mut names = BTreeSet::new();
    for (_, map) in &input_maps {
        for k in map.keys() {
            names.insert(k.clone());
        }
    }

    let mut out_entries = Vec::new();
    for name in names {
        let mut kinds: Vec<(String, Option<converge::model::ManifestEntryKind>)> = Vec::new();
        for (pub_id, map) in &input_maps {
            kinds.push((pub_id.clone(), map.get(&name).cloned()));
        }

        let all_present = kinds.iter().all(|(_, k)| k.is_some());
        if all_present {
            let all_dirs = kinds
                .iter()
                .all(|(_, k)| matches!(k, Some(converge::model::ManifestEntryKind::Dir { .. })));
            if all_dirs {
                let child_inputs = kinds
                    .iter()
                    .map(|(pub_id, k)| {
                        let converge::model::ManifestEntryKind::Dir { manifest } =
                            k.clone().unwrap()
                        else {
                            unreachable!();
                        };
                        (pub_id.clone(), manifest.as_str().to_string())
                    })
                    .collect::<Vec<_>>();
                let merged_child = merge_dir_manifests(state, repo_id, &child_inputs)?;
                out_entries.push(converge::model::ManifestEntry {
                    name,
                    kind: converge::model::ManifestEntryKind::Dir {
                        manifest: converge::model::ObjectId(merged_child),
                    },
                });
                continue;
            }

            // If all entry kinds are identical (file/symlink), keep it.
            let first = kinds[0].1.clone().unwrap();
            let identical = kinds.iter().all(|(_, k)| match k.clone().unwrap() {
                converge::model::ManifestEntryKind::File { .. } => k.clone().unwrap() == first,
                converge::model::ManifestEntryKind::FileChunks { .. } => {
                    k.clone().unwrap() == first
                }
                converge::model::ManifestEntryKind::Symlink { .. } => k.clone().unwrap() == first,
                _ => false,
            });
            if identical {
                out_entries.push(converge::model::ManifestEntry { name, kind: first });
                continue;
            }
        }

        // Conflict (or missing in some inputs): create a superposition entry.
        let mut variants = Vec::new();
        for (pub_id, kind) in kinds {
            let vkind = match kind {
                Some(converge::model::ManifestEntryKind::File { blob, mode, size }) => {
                    converge::model::SuperpositionVariantKind::File { blob, mode, size }
                }
                Some(converge::model::ManifestEntryKind::FileChunks { recipe, mode, size }) => {
                    converge::model::SuperpositionVariantKind::FileChunks { recipe, mode, size }
                }
                Some(converge::model::ManifestEntryKind::Dir { manifest }) => {
                    converge::model::SuperpositionVariantKind::Dir { manifest }
                }
                Some(converge::model::ManifestEntryKind::Symlink { target }) => {
                    converge::model::SuperpositionVariantKind::Symlink { target }
                }
                Some(converge::model::ManifestEntryKind::Superposition { variants }) => {
                    // Nested superposition: preserve as a variant to avoid losing information.
                    // Represent it by storing it as a derived manifest under a synthetic dir entry.
                    // For v1, treat as tombstone to force explicit resolution later.
                    let _ = variants;
                    converge::model::SuperpositionVariantKind::Tombstone
                }
                None => converge::model::SuperpositionVariantKind::Tombstone,
            };
            variants.push(converge::model::SuperpositionVariant {
                source: pub_id,
                kind: vkind,
            });
        }

        out_entries.push(converge::model::ManifestEntry {
            name,
            kind: converge::model::ManifestEntryKind::Superposition { variants },
        });
    }

    let merged = converge::model::Manifest {
        version: 1,
        entries: out_entries,
    };

    // Ensure references exist before persisting.
    for e in &merged.entries {
        // Bundles should be constructible even when blob bytes are pending.
        validate_manifest_entry_refs(state, repo_id, &e.kind, true)?;
    }

    store_manifest(state, repo_id, &merged)
}
