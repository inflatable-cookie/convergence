use super::super::*;

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
