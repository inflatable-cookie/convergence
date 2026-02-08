use super::super::*;
use super::store::{read_manifest, read_recipe};

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
